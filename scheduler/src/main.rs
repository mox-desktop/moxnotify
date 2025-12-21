pub mod moxnotify {
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
    pub mod common {
        tonic::include_proto!("moxnotify.common");
    }
    pub mod client {
        tonic::include_proto!("moxnotify.client");
    }
}

use crate::moxnotify::client::client_service_server::{ClientService, ClientServiceServer};
use crate::moxnotify::client::{
    ClientActionInvokedRequest, ClientActionInvokedResponse, ClientCloseNotificationRequest,
    ClientCloseNotificationResponse, ClientNotificationClosedRequest,
    ClientNotificationClosedResponse, ClientNotifyRequest,
};
use crate::moxnotify::types::CloseNotification;
use env_logger::Builder;
use log::LevelFilter;
use moxnotify::types::{NewNotification, NotificationMessage};
use redis::TypedCommands;
use redis::streams::StreamReadOptions;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, transport::Server};

#[derive(Clone)]
struct Scheduler {
    notification_broadcast: Arc<broadcast::Sender<NewNotification>>,
    close_notification_broadcast: Arc<broadcast::Sender<CloseNotification>>,
    redis_con: Arc<Mutex<redis::Connection>>,
}

impl Scheduler {
    fn new(redis_con: redis::Connection) -> Self {
        let (tx, _) = broadcast::channel(128);
        let (close_tx, _) = broadcast::channel(128);
        Self {
            notification_broadcast: Arc::new(tx),
            close_notification_broadcast: Arc::new(close_tx),
            redis_con: Arc::new(Mutex::new(redis_con)),
        }
    }
}

#[tonic::async_trait]
impl ClientService for Scheduler {
    type NotifyStream = Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<Item = Result<NotificationMessage, Status>>
                + Send
                + 'static,
        >,
    >;

    async fn notify(
        &self,
        request: Request<ClientNotifyRequest>,
    ) -> Result<Response<Self::NotifyStream>, Status> {
        let remote_addr = request.remote_addr().unwrap();
        log::info!("New client connection from: {:?}", remote_addr);

        let mut notification_rx = self.notification_broadcast.subscribe();
        let mut close_notification_rx = self.close_notification_broadcast.subscribe();
        let (tx, stream_rx) = mpsc::channel(128);

        let tx_clone = tx.clone();
        tokio::spawn(async move {
            loop {
                match notification_rx.recv().await {
                    Ok(notification) => {
                        let message = NotificationMessage {
                            notification: Some(notification),
                            close_notification: None,
                        };
                        if tx_clone.send(Ok(message)).await.is_err() {
                            log::info!("Client disconnected: {:?}", remote_addr);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        log::warn!(
                            "Client {:?} lagged, skipped {} notification messages",
                            remote_addr,
                            skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::error!(
                            "Notification broadcast channel closed for client: {:?}",
                            remote_addr
                        );
                        break;
                    }
                }
            }
        });

        tokio::spawn(async move {
            loop {
                match close_notification_rx.recv().await {
                    Ok(close_notification) => {
                        let message = NotificationMessage {
                            notification: None,
                            close_notification: Some(close_notification),
                        };
                        if tx.send(Ok(message)).await.is_err() {
                            log::info!("Client disconnected: {:?}", remote_addr);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        log::warn!(
                            "Client {:?} lagged, skipped {} close_notification messages",
                            remote_addr,
                            skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::error!(
                            "CloseNotification broadcast channel closed for client: {:?}",
                            remote_addr
                        );
                        break;
                    }
                }
            }
        });

        let output_stream: Self::NotifyStream = Box::pin(ReceiverStream::new(stream_rx));
        Ok(Response::new(output_stream))
    }

    async fn notification_closed(
        &self,
        request: Request<ClientNotificationClosedRequest>,
    ) -> Result<Response<ClientNotificationClosedResponse>, Status> {
        let closed = request.into_inner().notification_closed.unwrap();
        log::info!(
            "Received notification_closed request: id: {}, reason: {:?}",
            closed.id,
            closed.reason()
        );

        let mut con = self.redis_con.lock().unwrap();
        let json = serde_json::to_string(&closed).unwrap();
        if let Err(e) = con.xadd(
            "moxnotify:notification_closed",
            "*",
            &[("notification", json.as_str())],
        ) {
            log::error!("Failed to write notification_closed to Redis: {}", e);
        }

        Ok(Response::new(ClientNotificationClosedResponse {}))
    }

    async fn action_invoked(
        &self,
        request: Request<ClientActionInvokedRequest>,
    ) -> Result<Response<ClientActionInvokedResponse>, Status> {
        let invoked = request.into_inner().action_invoked.unwrap();
        log::info!(
            "Received action_invoked request: id: {}, key: {}",
            invoked.id,
            invoked.action_key
        );

        let mut con = self.redis_con.lock().unwrap();
        let json = serde_json::to_string(&invoked).unwrap();
        if let Err(e) = con.xadd(
            "moxnotify:action_invoked",
            "*",
            &[("action", json.as_str())],
        ) {
            log::error!("Failed to write action_invoked to Redis: {}", e);
        }

        Ok(Response::new(ClientActionInvokedResponse {}))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_level = LevelFilter::Info;
    Builder::new().filter(Some("scheduler"), log_level).init();

    let scheduler_addr =
        std::env::var("MOXNOTIFY_SCHEDULER_ADDR").unwrap_or_else(|_| "[::1]:50052".to_string());

    log::info!("Connecting to Redis and subscribing to notifications...");

    let client = redis::Client::open("redis://127.0.0.1/")?;
    let write_con = client.get_connection()?;
    let read_con = client.get_connection()?;
    let scheduler = Scheduler::new(write_con);
    let notification_broadcast = Arc::clone(&scheduler.notification_broadcast);
    let close_notification_broadcast = Arc::clone(&scheduler.close_notification_broadcast);

    let scheduler_clone = scheduler.clone();
    let server_addr = scheduler_addr.parse()?;
    tokio::spawn(async move {
        log::info!("Scheduler server listening on {}", server_addr);
        Server::builder()
            .add_service(ClientServiceServer::new(scheduler_clone))
            .serve(server_addr)
            .await
            .expect("Server failed to start");
    });

    log::info!("Subscribed to notifications from Redis stream");

    let mut con = read_con;
    loop {
        if let Some(streams) = con.xread_options(
            &["moxnotify:notify", "moxnotify:close_notification"],
            &[">", ">"],
            &StreamReadOptions::default()
                .group("scheduler-group", "scheduler-1")
                .block(0),
        )? {
            for stream_key in &streams.keys {
                match stream_key.key.as_str() {
                    "moxnotify:notify" => {
                        for stream_id in &stream_key.ids {
                            if let Some(redis::Value::BulkString(json)) =
                                stream_id.map.get("notification")
                            {
                                let json = std::str::from_utf8(json).unwrap();
                                let notification: NewNotification =
                                    serde_json::from_str(json).unwrap();

                                log::info!(
                                    "Scheduling notification: id={}, app_name='{}', summary='{}'",
                                    notification.id,
                                    notification.app_name,
                                    notification.summary
                                );

                                notification_broadcast.send(notification);

                                if let Err(e) = con.xack(
                                    "moxnotify:notify",
                                    "scheduler-group",
                                    &[stream_id.id.as_str()],
                                ) {
                                    log::error!("Failed to ACK message: {}", e);
                                }
                            }
                        }
                    }
                    "moxnotify:close_notification" => {
                        for stream_id in &stream_key.ids {
                            if let Some(redis::Value::BulkString(json)) =
                                stream_id.map.get("close_notification")
                            {
                                let json = std::str::from_utf8(json).unwrap();
                                let close_notification: CloseNotification =
                                    serde_json::from_str(json).unwrap();

                                log::info!(
                                    "Broadcasting close_notification to clients: id={}",
                                    close_notification.id
                                );

                                close_notification_broadcast.send(close_notification);

                                if let Err(e) = con.xack(
                                    "moxnotify:close_notification",
                                    "scheduler-group",
                                    &[stream_id.id.as_str()],
                                ) {
                                    log::error!("Failed to ACK message: {}", e);
                                }
                            }
                        }
                    }
                    _ => {
                        log::warn!("Received message from unknown stream: {}", stream_key.key);
                    }
                }
            }
        }
    }
}
