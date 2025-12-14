pub mod collector {
    tonic::include_proto!("collector");
}

pub mod indexer {
    tonic::include_proto!("indexer");
}

use collector::control_plane_server::{ControlPlane, ControlPlaneServer};
use collector::{CollectorMessage, ControlPlaneMessage};
use env_logger::Builder;
use indexer::control_plane_indexer_server::{ControlPlaneIndexer, ControlPlaneIndexerServer};
use indexer::{IndexerNotificationMessage, IndexerSubscribeRequest};
use log::LevelFilter;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, transport::Server};

#[derive(Clone)]
pub struct ControlPlaneService {
    active_connections: Arc<Mutex<HashMap<SocketAddr, ConnectionInfo>>>,
    notification_broadcast: Arc<broadcast::Sender<IndexerNotificationMessage>>,
}

impl ControlPlaneService {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(128);
        Self {
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            notification_broadcast: Arc::new(tx),
        }
    }
}

struct ConnectionInfo {
    connected_at: std::time::SystemTime,
}

#[tonic::async_trait]
impl ControlPlane for ControlPlaneService {
    type NotificationsStream = Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<Item = Result<ControlPlaneMessage, Status>>
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
        let (_tx, rx) = mpsc::channel(128);
        let notification_broadcast = Arc::clone(&self.notification_broadcast);

        tokio::spawn(async move {
            while let Some(msg_result) = stream.next().await {
                match msg_result {
                    Ok(msg) => {
                        match msg.message {
                            Some(collector::collector_message::Message::NewNotification(
                                notification,
                            )) => {
                                log::info!(
                                    "Received notification: id={}, app_name='{}', summary='{}', body='{}'",
                                    notification.id,
                                    notification.app_name,
                                    notification.summary,
                                    notification.body,
                                );

                                let indexer_notification = indexer::NewNotification {
                                    id: notification.id,
                                    app_name: notification.app_name,
                                    app_icon: notification.app_icon,
                                    summary: notification.summary,
                                    body: notification.body,
                                    timeout: notification.timeout,
                                    actions: notification
                                        .actions
                                        .into_iter()
                                        .map(|a| indexer::Action {
                                            key: a.key,
                                            label: a.label,
                                        })
                                        .collect(),
                                    hints: notification.hints.map(|h| indexer::NotificationHints {
                                        action_icons: h.action_icons,
                                        category: h.category,
                                        value: h.value,
                                        desktop_entry: h.desktop_entry,
                                        resident: h.resident,
                                        sound_file: h.sound_file,
                                        sound_name: h.sound_name,
                                        suppress_sound: h.suppress_sound,
                                        transient: h.transient,
                                        x: h.x,
                                        y: h.y,
                                        urgency: h.urgency,
                                        image: h.image.map(|img| indexer::Image {
                                            image: img.image.map(|i| match i {
                                                collector::image::Image::Name(name) => {
                                                    indexer::image::Image::Name(name)
                                                }
                                                collector::image::Image::FilePath(path) => {
                                                    indexer::image::Image::FilePath(path)
                                                }
                                                collector::image::Image::Data(data) => {
                                                    indexer::image::Image::Data(
                                                        indexer::ImageData {
                                                            width: data.width,
                                                            height: data.height,
                                                            rowstride: data.rowstride,
                                                            has_alpha: data.has_alpha,
                                                            bits_per_sample: data.bits_per_sample,
                                                            channels: data.channels,
                                                            data: data.data,
                                                        },
                                                    )
                                                }
                                            }),
                                        }),
                                    }),
                                };

                                let message = IndexerNotificationMessage {
                                    notification: Some(indexer_notification),
                                };

                                let _ = notification_broadcast.send(message);

                                // TODO: Route notification to frontend
                            }
                            Some(collector::collector_message::Message::NotificationClosed(
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
impl ControlPlaneIndexer for ControlPlaneService {
    type StreamNotificationsStream = Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<
                    Item = Result<IndexerNotificationMessage, Status>,
                > + Send
                + 'static,
        >,
    >;

    async fn stream_notifications(
        &self,
        _request: Request<IndexerSubscribeRequest>,
    ) -> Result<Response<Self::StreamNotificationsStream>, Status> {
        let remote_addr = _request.remote_addr().unwrap();
        log::info!("New indexer connection from: {:?}", remote_addr);

        let mut rx = self.notification_broadcast.subscribe();
        let (tx, stream_rx) = mpsc::channel(128);

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        if tx.send(Ok(msg)).await.is_err() {
                            log::info!("Indexer client disconnected: {:?}", remote_addr);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        log::warn!(
                            "Indexer {:?} lagged, skipped {} messages",
                            remote_addr,
                            skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::error!("Broadcast channel closed for indexer: {:?}", remote_addr);
                        break;
                    }
                }
            }
        });

        let output_stream: Self::StreamNotificationsStream =
            Box::pin(ReceiverStream::new(stream_rx));
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
        .add_service(ControlPlaneServer::new(service.clone()))
        .add_service(ControlPlaneIndexerServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
