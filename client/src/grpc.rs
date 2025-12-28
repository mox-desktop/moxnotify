use crate::Event;
use crate::moxnotify::client::client_service_client::ClientServiceClient;
use crate::moxnotify::client::{ClientNotifyRequest, notification_message};
use futures_lite::stream::StreamExt;
use tokio::time;
use tonic::Request;
use tonic::transport::Channel;

pub async fn serve(
    mut client: ClientServiceClient<Channel>,
    event_sender: calloop::channel::Sender<Event>,
    max_visible: u32,
) -> anyhow::Result<()> {
    let mut disconnects = 0;
    loop {
        let request = Request::new(ClientNotifyRequest { max_visible });
        let mut stream = client.notify(request).await.unwrap().into_inner();

        log::info!("Connected to scheduler, subscribing to notifications...");

        while let Some(msg_result) = stream.next().await {
            if let Ok(msg) = msg_result
                && let Some(message) = msg.message
            {
                disconnects = 0;
                match message {
                    notification_message::Message::Notification(notification) => {
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
                    notification_message::Message::CloseNotification(close_notification) => {
                        log::info!("Received close_notification: id={}", close_notification.id);

                        if let Err(e) =
                            event_sender.send(Event::CloseNotification(close_notification.id))
                        {
                            log::error!("Error: {e}");
                        }
                    }
                }
            }
        }

        log::error!("Disconnected from scheduler, trying to reconnect...");

        time::sleep(time::Duration::from_secs(disconnects)).await;
        disconnects += 1;
        let backoff = std::cmp::min(30, 2_u64.pow(disconnects as u32));
        log::info!("Reconnecting in {} seconds...", backoff);
    }
}
