pub mod moxnotify {
    pub mod common {
        tonic::include_proto!("moxnotify.common");
    }
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
    pub mod scheduler {
        tonic::include_proto!("moxnotify.scheduler");
    }
    pub mod client {
        tonic::include_proto!("moxnotify.client");
    }
}

use crate::moxnotify::client::client_service_server::{ClientService, ClientServiceServer};
use crate::moxnotify::client::{ClientNotifyMessage, ClientNotifyRequest};
use crate::moxnotify::scheduler::{
    SchedulerSubscribeRequest, scheduler_service_client::SchedulerServiceClient,
};
use env_logger::Builder;
use log::LevelFilter;
use moxnotify::types::NewNotification;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, transport::Server};

#[derive(Clone)]
struct Scheduler {
    notification_broadcast: Arc<broadcast::Sender<NewNotification>>,
}

impl Scheduler {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(128);
        Self {
            notification_broadcast: Arc::new(tx),
        }
    }
}

#[tonic::async_trait]
impl ClientService for Scheduler {
    type NotifyStream = Pin<
        Box<
            dyn tonic::codegen::tokio_stream::Stream<Item = Result<ClientNotifyMessage, Status>>
                + Send
                + 'static,
        >,
    >;

    async fn notify(
        &self,
        _request: Request<ClientNotifyRequest>,
    ) -> Result<Response<Self::NotifyStream>, Status> {
        let remote_addr = _request.remote_addr().unwrap();
        log::info!("New client connection from: {:?}", remote_addr);

        let mut rx = self.notification_broadcast.subscribe();
        let (tx, stream_rx) = mpsc::channel(128);

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(notification) => {
                        let message = ClientNotifyMessage {
                            notification: Some(notification),
                        };
                        if tx.send(Ok(message)).await.is_err() {
                            log::info!("Client disconnected: {:?}", remote_addr);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        log::warn!(
                            "Client {:?} lagged, skipped {} messages",
                            remote_addr,
                            skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::error!("Broadcast channel closed for client: {:?}", remote_addr);
                        break;
                    }
                }
            }
        });

        let output_stream: Self::NotifyStream = Box::pin(ReceiverStream::new(stream_rx));
        Ok(Response::new(output_stream))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_level = LevelFilter::Info;
    Builder::new().filter(Some("scheduler"), log_level).init();

    let scheduler_addr =
        std::env::var("MOXNOTIFY_SCHEDULER_ADDR").unwrap_or_else(|_| "[::1]:50052".to_string());
    let control_plane_addr = std::env::var("MOXNOTIFY_CONTROL_PLANE_ADDR")
        .unwrap_or_else(|_| "http://[::1]:50051".to_string());

    let scheduler = Scheduler::new();
    let notification_broadcast = Arc::clone(&scheduler.notification_broadcast);

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

    log::info!("Connecting to control plane at: {}", control_plane_addr);

    let mut client = SchedulerServiceClient::connect(control_plane_addr).await?;

    log::info!("Connected to control plane, subscribing to notifications...");

    let request = Request::new(SchedulerSubscribeRequest {});
    let mut stream = client.stream_notifications(request).await?.into_inner();

    log::info!("Subscribed to notifications");

    while let Some(msg_result) = stream.next().await {
        if let Ok(msg) = msg_result
            && let Some(notification) = msg.notification
        {
            log::info!(
                "Scheduling notification: id={}, app_name='{}', summary='{}', body='{}', urgency='{}'",
                notification.id,
                notification.app_name,
                notification.summary,
                notification.body,
                notification.hints.as_ref().unwrap().urgency
            );

            // Forward notification to clients
            let _ = notification_broadcast.send(notification);
        }
    }

    Ok(())
}
