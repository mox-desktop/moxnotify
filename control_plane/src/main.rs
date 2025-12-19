pub mod moxnotify {
    pub mod common {
        tonic::include_proto!("moxnotify.common");
    }
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
    pub mod collector {
        tonic::include_proto!("moxnotify.collector");
    }
    pub mod scheduler {
        tonic::include_proto!("moxnotify.scheduler");
    }
}

use env_logger::Builder;
use log::LevelFilter;
use moxnotify::collector::collector_service_server::{CollectorService, CollectorServiceServer};
use moxnotify::collector::{CollectorMessage, CollectorResponse};
use moxnotify::scheduler::SchedulerSubscribeRequest;
use moxnotify::scheduler::scheduler_service_server::{SchedulerService, SchedulerServiceServer};
use moxnotify::types::{NewNotification, NotificationMessage};
use redis::TypedCommands;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, transport::Server};
use uuid::Uuid;

use crate::moxnotify::scheduler::{
    SchedulerNotificationClosedRequest, SchedulerNotificationClosedResponse,
};

#[derive(Clone)]
pub struct ControlPlaneService {
    client: redis::Client,
    con: Arc<Mutex<redis::Connection>>,
    active_connections: Arc<Mutex<HashMap<SocketAddr, ConnectionInfo>>>,
    notification_broadcast: Arc<broadcast::Sender<NewNotification>>,
}

impl ControlPlaneService {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(128);

        let client = redis::Client::open("redis://127.0.0.1/").unwrap();
        let mut con = client.get_connection().unwrap();

        con.xgroup_create_mkstream("moxnotify:notify", "indexer-group", "$");
        con.xgroup_create_mkstream("moxnotify:notify", "scheduler-group", "$");
        con.xgroup_create_mkstream("moxnotify:notification_closed", "scheduler-group", "$");

        Self {
            client,
            con: Arc::new(Mutex::new(con)),
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            notification_broadcast: Arc::new(tx),
        }
    }
}

struct ConnectionInfo {
    connected_at: std::time::SystemTime,
}

#[tonic::async_trait]
impl CollectorService for ControlPlaneService {
    type NotificationsStream = Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<Item = Result<CollectorResponse, Status>>
                + Send
                + 'static,
        >,
    >;

    async fn notifications(
        &self,
        request: Request<tonic::Streaming<CollectorMessage>>,
    ) -> Result<Response<Self::NotificationsStream>, Status> {
        let remote_addr = request.remote_addr().unwrap();

        log::info!("New connection from: {:?}", remote_addr);

        let active_connections = Arc::clone(&self.active_connections);

        {
            let mut active_connections = active_connections.lock().unwrap();

            let conn_info = ConnectionInfo {
                connected_at: std::time::SystemTime::now(),
            };
            active_connections.insert(remote_addr, conn_info);
        }

        let mut stream = request.into_inner();
        let (tx, rx) = mpsc::channel(128);
        let notification_broadcast = Arc::clone(&self.notification_broadcast);

        // Send initial connection acknowledgment
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let ack = CollectorResponse {
                message: Some(
                    moxnotify::collector::collector_response::Message::ConnectionRegistered(
                        moxnotify::collector::ConnectionRegistered {
                            message: Uuid::new_v4().to_string(),
                        },
                    ),
                ),
            };
            if let Err(e) = tx_clone.send(Ok(ack)).await {
                log::warn!("Failed to send connection acknowledgment: {}", e);
            }
        });

        let con = Arc::clone(&self.con);
        tokio::spawn(async move {
            let _response_tx = tx; // Available for sending responses to collector if needed
            while let Some(msg_result) = stream.next().await {
                match msg_result {
                    Ok(msg) => {
                        match msg.message {
                            Some(moxnotify::collector::collector_message::Message::NewNotification(
                                notification,
                            )) => {
                                log::info!(
                                    "Received notification: id={}, app_name='{}', summary='{}', body='{}', urgency='{}'",
                                    notification.id,
                                    notification.app_name,
                                    notification.summary,
                                    notification.body,
                                    notification.hints.as_ref().unwrap().urgency
                                );

                                let mut con = con.lock().unwrap();
                                let json = serde_json::to_string(&notification).unwrap();
                                con.xadd("moxnotify:notify", "*", &[("notification", json.as_str())])
                                    .unwrap();

                                let _ = notification_broadcast.send(notification.clone());
                            }
                            Some(moxnotify::collector::collector_message::Message::NotificationClosed(
                                closed,
                            )) => {
                                log::info!(
                                    "Notification closed: id={}, reason={:?}",
                                    closed.id,
                                    closed.reason()
                                );
                                // TODO: Notify frontend
                            }
                            None => {
                                log::warn!("Received empty CollectorMessage");
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Error receiving message from collector: {}", e);
                        break;
                    }
                }
            }

            {
                let active_connections = active_connections.lock().unwrap();
                if let Some(conn_info) = active_connections.get(&remote_addr) {
                    log::info!(
                        "Client disconnected, addr: {:?}, active for: {:?}",
                        remote_addr,
                        conn_info.connected_at.elapsed().unwrap_or_default()
                    );
                } else {
                    log::error!("Client disconnected twice, addr: {:?}", remote_addr);
                }
            }
        });

        let output_stream: Self::NotificationsStream = Box::pin(ReceiverStream::new(rx));
        Ok(Response::new(output_stream))
    }
}

#[tonic::async_trait]
impl SchedulerService for ControlPlaneService {
    type StreamNotificationsStream = Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<Item = Result<NotificationMessage, Status>>
                + Send
                + 'static,
        >,
    >;

    async fn stream_notifications(
        &self,
        request: Request<SchedulerSubscribeRequest>,
    ) -> Result<Response<Self::StreamNotificationsStream>, Status> {
        let remote_addr = request.remote_addr().unwrap();
        log::info!("New scheduler connection from: {:?}", remote_addr);

        let mut rx = self.notification_broadcast.subscribe();
        let (tx, stream_rx) = mpsc::channel(128);

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(notification) => {
                        let message = NotificationMessage {
                            notification: Some(notification),
                        };
                        if tx.send(Ok(message)).await.is_err() {
                            log::info!("Scheduler client disconnected: {:?}", remote_addr);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        log::warn!(
                            "Scheduler {:?} lagged, skipped {} messages",
                            remote_addr,
                            skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::error!("Broadcast channel closed for scheduler: {:?}", remote_addr);
                        break;
                    }
                }
            }
        });

        let output_stream: Self::StreamNotificationsStream =
            Box::pin(ReceiverStream::new(stream_rx));
        Ok(Response::new(output_stream))
    }

    async fn notification_closed(
        &self,
        _request: Request<SchedulerNotificationClosedRequest>,
    ) -> Result<Response<SchedulerNotificationClosedResponse>, Status> {
        let output_stream = SchedulerNotificationClosedResponse {};

        let _connections = self.active_connections.lock().unwrap();

        Ok(Response::new(output_stream))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_level = LevelFilter::Info;
    Builder::new()
        .filter(Some("control_plane"), log_level)
        .init();

    let addr = "[::1]:50051".parse()?;
    let service = ControlPlaneService::new();

    log::info!("Control plane server listening on {}", addr);

    Server::builder()
        .add_service(CollectorServiceServer::new(service.clone()))
        .add_service(SchedulerServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
