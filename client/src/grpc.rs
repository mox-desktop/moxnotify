use crate::{
    Event,
    moxnotify::client::{ClientNotifyRequest, client_service_client::ClientServiceClient},
};
use futures_lite::stream::StreamExt;
use tonic::{Request, transport::Channel};

pub async fn serve(
    mut client: ClientServiceClient<Channel>,
    event_sender: calloop::channel::Sender<Event>,
) -> anyhow::Result<()> {
    log::info!("Connected to scheduler, subscribing to notifications...");

    let request = Request::new(ClientNotifyRequest {});
    let mut stream = client.notify(request).await.unwrap().into_inner();

    log::info!("Subscribed to notifications");

    while let Some(msg_result) = stream.next().await {
        if let Ok(msg) = msg_result {
            if let Some(notification) = msg.notification {
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
            } else if let Some(close_notification) = msg.close_notification {
                log::info!("Received close_notification: id={}", close_notification.id);

                if let Err(e) = event_sender.send(Event::CloseNotification(close_notification.id)) {
                    log::error!("Error: {e}");
                }
            }
        }
    }

    Ok(())
}
