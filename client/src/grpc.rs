use crate::{
    EmitEvent, Event,
    moxnotify::{
        client::{
            ClientNotificationClosedRequest, ClientNotifyRequest,
            client_service_client::ClientServiceClient,
        },
        types::NotificationClosed,
    },
};
use futures_lite::stream::StreamExt;
use tokio::sync::broadcast;
use tonic::Request;

pub async fn serve(
    event_sender: calloop::channel::Sender<Event>,
    mut emit_receiver: broadcast::Receiver<EmitEvent>,
) -> anyhow::Result<()> {
    let scheduler_addr = std::env::var("MOXNOTIFY_SCHEDULER_ADDR")
        .unwrap_or_else(|_| "http://[::1]:50052".to_string());

    log::info!("Connecting to scheduler at: {}", scheduler_addr);

    let mut client = ClientServiceClient::connect(scheduler_addr).await.unwrap();

    log::info!("Connected to scheduler, subscribing to notifications...");

    let request = Request::new(ClientNotifyRequest {});
    let mut stream = client.notify(request).await.unwrap().into_inner();

    log::info!("Subscribed to notifications");

    tokio::spawn(async move {
        while let Some(msg_result) = stream.next().await {
            if let Ok(msg) = msg_result
                && let Some(notification) = msg.notification
            {
                log::info!(
                    "Received notification: id={}, app_name='{}', summary='{}', body='{}', urgency='{}'",
                    notification.id,
                    notification.app_name,
                    notification.summary,
                    notification.body,
                    notification.hints.as_ref().unwrap().urgency
                );

                if let Err(e) = event_sender.send(Event::Notify(Box::new(notification))) {
                    log::error!("Error: {e}");
                }
            }
        }
    });

    while let Ok(event) = emit_receiver.recv().await {
        match event {
            EmitEvent::NotificationClosed { id, reason, uuid } => {
                log::info!("Notification dismissed: id: {}, reason: {}", id, reason);
                client
                    .notification_closed(Request::new(ClientNotificationClosedRequest {
                        notification_closed: Some(NotificationClosed {
                            id,
                            reason: reason as i32,
                            uuid,
                        }),
                    }))
                    .await
                    .unwrap();
            }
            EmitEvent::ActionInvoked {
                id: _,
                key: _,
                token: _,
            } => {}
            _ => {}
        }
    }

    Ok(())
}
