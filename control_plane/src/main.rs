mod history;
pub mod collector {
    tonic::include_proto!("collector");
}

use collector::control_plane_server::{ControlPlane, ControlPlaneServer};
use collector::{CollectorMessage, ControlPlaneMessage};
use env_logger::Builder;
use log::LevelFilter;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, transport::Server};

// TODO: set through config, also change paths
fn path() -> PathBuf {
    let path = std::env::var("XDG_DATA_HOME")
        .map(|data_home| PathBuf::from(data_home).join("moxnotify-control-plane/db.mox"))
        .or_else(|_| {
            std::env::var("HOME")
                .map(|home| PathBuf::from(home).join(".local/share/moxnotify-control-plane/db.mox"))
        })
        .unwrap_or_else(|_| PathBuf::from(""));

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }

    path
}

pub struct ControlPlaneService {
    history: Arc<Mutex<history::History>>,
    active_connections: Arc<Mutex<HashMap<SocketAddr, ConnectionInfo>>>,
}

impl ControlPlaneService {
    fn new() -> Self {
        Self {
            history: Arc::new(Mutex::new(history::History::try_new(&path()).unwrap())),
            active_connections: Arc::new(Mutex::new(HashMap::new())),
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

        let history = Arc::clone(&self.history);

        tokio::spawn(async move {
            while let Some(msg_result) = stream.next().await {
                match msg_result {
                    Ok(msg) => {
                        match msg.message {
                            Some(collector::collector_message::Message::NewNotification(
                                notification,
                            )) => {
                                let image_desc = notification
                                    .hints
                                    .as_ref()
                                    .and_then(|h| h.image.as_ref())
                                    .and_then(|img| img.image.as_ref())
                                    .map(|image| match image {
                                        collector::image::Image::Name(name) => {
                                            format!("Name({name})")
                                        }
                                        collector::image::Image::FilePath(path) => {
                                            format!("File({path})")
                                        }
                                        collector::image::Image::Data(data) => {
                                            format!("Data({}x{})", data.width, data.height)
                                        }
                                    });

                                log::info!(
                                    "Received notification: id={}, app_name='{}', app_icon={:?}, summary='{}', body='{}', timeout={}, actions={:?}, hints={{ urgency={:?}, category={:?}, desktop_entry={:?}, resident={}, transient={}, suppress_sound={}, action_icons={}, x={}, y={:?}, value={:?}, sound_file={:?}, sound_name={:?}, image={:?} }}",
                                    notification.id,
                                    notification.app_name,
                                    notification.app_icon,
                                    notification.summary,
                                    notification.body,
                                    notification.timeout,
                                    notification.actions,
                                    notification.hints.as_ref().unwrap().urgency,
                                    notification.hints.as_ref().unwrap().category,
                                    notification.hints.as_ref().unwrap().desktop_entry,
                                    notification.hints.as_ref().unwrap().resident,
                                    notification.hints.as_ref().unwrap().transient,
                                    notification.hints.as_ref().unwrap().suppress_sound,
                                    notification.hints.as_ref().unwrap().action_icons,
                                    notification.hints.as_ref().unwrap().x,
                                    notification.hints.as_ref().unwrap().y,
                                    notification.hints.as_ref().unwrap().value,
                                    notification.hints.as_ref().unwrap().sound_file.as_ref(),
                                    notification.hints.as_ref().unwrap().sound_name,
                                    image_desc,
                                );

                                let history = history.lock().unwrap();
                                if let Err(e) = history.insert(&notification) {
                                    log::error!("Failed to insert notification into database");
                                }

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
        .add_service(ControlPlaneServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
