pub mod moxnotify {
    pub mod common {
        tonic::include_proto!("moxnotify.common");
    }
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
}
mod indexer {
    tonic::include_proto!("scheduler");
}

use env_logger::Builder;
use log::LevelFilter;
use tokio_stream::StreamExt;
use tonic::Request;

use crate::indexer::{
    SchedulerSubscribeRequest, control_plane_scheduler_client::ControlPlaneSchedulerClient,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_level = LevelFilter::Info;
    Builder::new().filter(Some("scheduler"), log_level).init();

    let control_plane_addr = std::env::var("MOXNOTIFY_CONTROL_PLANE_ADDR")
        .unwrap_or_else(|_| "http://[::1]:50051".to_string());

    log::info!("Connecting to control plane at: {}", control_plane_addr);

    let mut client = ControlPlaneSchedulerClient::connect(control_plane_addr).await?;

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
        }
    }

    Ok(())
}
