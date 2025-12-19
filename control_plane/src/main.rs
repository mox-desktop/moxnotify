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
}

use crate::moxnotify::types::NotificationClosed;
use env_logger::Builder;
use log::LevelFilter;
use moxnotify::collector::collector_service_server::{CollectorService, CollectorServiceServer};
use moxnotify::collector::{CollectorMessage, CollectorResponse};
use moxnotify::types::NewNotification;
use redis::TypedCommands;
use redis::streams::StreamReadOptions;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, transport::Server};

#[derive(Clone)]
pub struct ControlPlaneService {
    con: Arc<Mutex<redis::Connection>>,
    notification_broadcast: Arc<broadcast::Sender<NewNotification>>,
}

impl ControlPlaneService {
    fn try_new(mut redis_con: redis::Connection) -> anyhow::Result<Self> {
        let (tx, _) = broadcast::channel(128);

        // If any of these errors it's likely because group already exists
        _ = redis_con.xgroup_create_mkstream("moxnotify:notify", "indexer-group", "$");
        _ = redis_con.xgroup_create_mkstream("moxnotify:notify", "scheduler-group", "$");
        _ = redis_con.xgroup_create_mkstream(
            "moxnotify:notification_closed",
            "control-plane-group",
            "$",
        );

        Ok(Self {
            con: Arc::new(Mutex::new(redis_con)),
            notification_broadcast: Arc::new(tx),
        })
    }
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

        let mut stream = request.into_inner();
        let (tx, rx) = mpsc::channel(128);
        let notification_broadcast = Arc::clone(&self.notification_broadcast);

        let con = Arc::clone(&self.con);
        tokio::spawn(async move {
            let _response_tx = tx;
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

                                let mut con = con.lock().await;
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

            log::info!("Client disconnected, addr: {:?}", remote_addr,);
        });

        let output_stream: Self::NotificationsStream = Box::pin(ReceiverStream::new(rx));
        Ok(Response::new(output_stream))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_level = LevelFilter::Info;
    Builder::new()
        .filter(Some("control_plane"), log_level)
        .init();

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();

    let addr = "[::1]:50051".parse()?;
    let service = ControlPlaneService::try_new(client.get_connection()?)?;

    log::info!("Control plane server listening on {}", addr);

    let con = client.get_connection().unwrap();
    tokio::spawn(async move {
        let mut con = con;
        loop {
            if let Some(streams) = con
                .xread_options(
                    &["moxnotify:notification_closed"],
                    &[">"],
                    &StreamReadOptions::default()
                        .group("control-plane-group", "control-plane")
                        .block(0),
                )
                .unwrap()
                && let Some(stream_key) = streams
                    .keys
                    .iter()
                    .find(|sk| sk.key == "moxnotify:notification_closed")
            {
                for stream_id in &stream_key.ids {
                    if let Some(redis::Value::BulkString(json)) = stream_id.map.get("notification")
                    {
                        match std::str::from_utf8(json) {
                            Ok(json_str) => {
                                match serde_json::from_str::<NotificationClosed>(json_str) {
                                    Ok(closed) => {
                                        log::info!(
                                            "Received notification_closed from Redis: id: {}, reason: {:?}",
                                            closed.id,
                                            closed.reason()
                                        );

                                        if let Err(e) = con.xack(
                                            "moxnotify:notification_closed",
                                            "control-plane-group",
                                            &[stream_id.id.as_str()],
                                        ) {
                                            log::error!("Failed to ACK message: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "Failed to parse notification_closed from Redis: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to convert notification_closed bytes to string: {}",
                                    e
                                );
                            }
                        }
                    } else {
                        log::warn!(
                            "Received notification_closed message from Redis but 'notification' field is missing or has unexpected type: stream_id={}",
                            stream_id.id
                        );
                    }
                }
            }
        }
    });

    Server::builder()
        .add_service(CollectorServiceServer::new(service.clone()))
        .serve(addr)
        .await?;

    Ok(())
}
