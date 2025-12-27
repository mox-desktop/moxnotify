pub mod moxnotify {
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
    pub mod collector {
        tonic::include_proto!("moxnotify.collector");
    }
}

mod dbus;
mod image_data;

use moxnotify::collector::CollectorMessage;
use moxnotify::collector::collector_service_client::CollectorServiceClient;
use moxnotify::collector::{collector_message, collector_response};
use moxnotify::types::{ActionInvoked, CloseNotification, NewNotification, NotificationClosed};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

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
    env_logger::Builder::from_env(env_logger::Env::new().filter("MOXNOTIFY_LOG"))
        .filter_level(log::LevelFilter::Off)
        .filter_module("collector", log::LevelFilter::max())
        .init();

    let (event_sender, mut event_receiver) = mpsc::channel(128);
    let (emit_sender, emit_receiver) = broadcast::channel(128);

    tokio::spawn(async move {
        let uuid = Uuid::new_v4().to_string();
        if let Err(e) = dbus::serve(event_sender, emit_receiver, uuid).await {
            log::error!("D-Bus serve error: {e}");
        }
    });

    let addr = "http://[::1]:50051";
    let mut client = CollectorServiceClient::connect(addr.to_string()).await?;
    log::info!("Connected to control plane at {}", addr);

    let (tx, rx) = mpsc::channel(128);
    let message_stream = ReceiverStream::new(rx);

    let mut response_stream = client.notifications(message_stream).await?.into_inner();

    loop {
        tokio::select! {
            event = event_receiver.recv() => {
                let Some(event) = event else {
                    log::info!("Event receiver closed");
                    break;
                };

                let msg = match event {
                    Event::Notify(data) => {
                        log::info!(
                            "Collected notification: id={}, app_name='{}', summary='{}'",
                            data.id,
                            data.app_name,
                            data.summary,
                        );

                        CollectorMessage {
                            message: Some(collector_message::Message::NewNotification(*data)),
                        }
                    }
                    Event::CloseNotification(id) => {
                        log::info!("Collected close notification request: id={}", id);

                        CollectorMessage {
                            message: Some(collector_message::Message::CloseNotification(
                                CloseNotification { id },
                            )),
                        }
                    }
                };

                if let Err(e) = tx.send(msg).await {
                    log::error!("Failed to send message to control plane: {e}");
                    break;
                }
            }

            response = response_stream.next() => {
                match response {
                    Some(Ok(response)) => {
                        if let Some(msg) = response.message {
                            match msg {
                                collector_response::Message::ActionInvoked(action) => {
                                    log::info!(
                                        "Received action invoked: id={}, action_key='{}'",
                                        action.id,
                                        action.action_key
                                    );

                                    if let Err(e) =
                                        emit_sender.send(EmitEvent::ActionInvoked(action))
                                    {
                                        log::warn!(
                                            "Failed to forward action invoked to DBus emitter: {}",
                                            e
                                        );
                                    }
                                }
                                collector_response::Message::NotificationClosed(closed) => {
                                    log::info!(
                                        "Received notification closed: id={}, reason={:?}",
                                        closed.id,
                                        closed.reason()
                                    );

                                    if let Err(e) =
                                        emit_sender.send(EmitEvent::NotificationClosed(closed))
                                    {
                                        log::warn!(
                                            "Failed to forward notification closed to DBus emitter: {}",
                                            e
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        log::error!("Error receiving response from control plane: {}", e);
                        break;
                    }
                    None => {
                        log::info!("Response stream ended");
                        break;
                    }
                }
            }
        }
    }

    log::info!("Main event loop exited");

    Ok(())
}
