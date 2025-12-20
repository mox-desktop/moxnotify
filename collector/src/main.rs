mod dbus;
mod image_data;
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

use env_logger::Builder;
use log::LevelFilter;
use moxnotify::collector::CollectorMessage;
use moxnotify::collector::collector_message::Message;
use moxnotify::collector::collector_service_client::CollectorServiceClient;
use moxnotify::types::{ActionInvoked, NewNotification, NotificationClosed};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

type NotificationId = u32;

#[derive(Debug)]
pub enum Event {
    Notify(Box<NewNotification>),
    CloseNotification(NotificationId),
}

#[derive(Clone)]
pub enum EmitEvent {
    ActionInvoked(ActionInvoked),
    NotificationClosed(NotificationClosed),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_level = LevelFilter::Info;
    Builder::new().filter(Some("collector"), log_level).init();

    let (event_sender, mut event_receiver) = mpsc::channel(128);
    let (emit_sender, emit_receiver) = broadcast::channel(128);

    tokio::spawn(async move {
        let uuid = "test".to_string();
        if let Err(e) = dbus::serve(event_sender, emit_receiver, uuid).await {
            log::error!("D-Bus serve error: {e}");
        }
    });

    let addr = "http://[::1]:50051";
    let mut client = CollectorServiceClient::connect(addr.to_string()).await?;
    log::info!("Connected to control plane at {}", addr);

    let (tx, rx) = mpsc::channel(128);
    let message_stream = ReceiverStream::new(rx);
    let emit_sender_clone = emit_sender.clone();

    let mut response_stream = client.notifications(message_stream).await?.into_inner();
    tokio::spawn(async move {
        while let Some(response_result) = response_stream.next().await {
            match response_result {
                Ok(response) => {
                    if let Some(msg) = response.message {
                        match msg {
                            moxnotify::collector::collector_response::Message::ActionInvoked(action) => {
                                log::info!("Received action invoked: id={}, action_key='{}'", action.id, action.action_key);
                                let _ = emit_sender_clone.send(EmitEvent::ActionInvoked(action));
                            }
                            moxnotify::collector::collector_response::Message::NotificationClosed(closed) => {
                                log::info!("Received notification closed from control plane: id={}, reason={:?}, forwarding to DBus", closed.id, closed.reason());
                                if let Err(e) = emit_sender_clone.send(EmitEvent::NotificationClosed(closed)) {
                                    log::warn!("Failed to forward notification closed to DBus emitter: {}", e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Error receiving response from control plane: {}", e);
                    break;
                }
            }
        }
        log::info!("Response stream ended");
    });

    while let Some(event) = event_receiver.recv().await {
        let msg = match event {
            Event::Notify(data) => {
                log::info!(
                    "Collected notification: id={}, app_name='{}', summary='{}', body='{}', urgency='{}', timeout={}, actions={:?}",
                    data.id,
                    data.app_name,
                    data.summary,
                    data.body,
                    data.hints.as_ref().unwrap().urgency,
                    data.timeout,
                    data.actions,
                );

                CollectorMessage {
                    message: Some(Message::NewNotification(*data)),
                }
            }
            Event::CloseNotification(_id) => {
                continue;
            }
        };

        if let Err(e) = tx.send(msg).await {
            log::error!("Failed to send message to control plane: {e}");
            break;
        }
    }

    Ok(())
}
