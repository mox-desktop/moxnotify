mod dbus;
mod image_data;
pub mod collector {
    tonic::include_proto!("collector");
}

use calloop::EventLoop;
use collector::collector_message::Message;
use collector::control_plane_client::ControlPlaneClient;
use collector::{
    ActionInvoked, CloseReason, CollectorMessage, NewNotification, NotificationClosed,
};
use env_logger::Builder;
use log::LevelFilter;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;

type NotificationId = u32;

struct Collector {
    #[allow(dead_code)]
    client: ControlPlaneClient<Channel>,
    message_sender: mpsc::Sender<CollectorMessage>,
}

impl Collector {
    async fn handle_app_event(&mut self, event: Event) -> anyhow::Result<()> {
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
            Event::CloseNotification(id) => CollectorMessage {
                message: Some(Message::NotificationClosed(NotificationClosed {
                    id,
                    reason: CloseReason::ReasonCloseNotificationCall as i32,
                })),
            },
        };

        self.message_sender
            .send(msg)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send message: {e}"))?;

        Ok(())
    }
}

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

    let mut client = ControlPlaneClient::connect("http://[::1]:50051")
        .await
        .unwrap();

    let (tx, rx) = mpsc::channel(128);
    let message_sender = tx;

    let response = client.notifications(ReceiverStream::new(rx)).await?;
    let mut incoming_stream = response.into_inner();

    let mut collector = Collector {
        client,
        message_sender,
    };

    let mut event_loop = EventLoop::try_new()?;

    let (executor, scheduler) = calloop::futures::executor()?;
    let (event_sender, event_receiver) = calloop::channel::channel();
    let (emit_sender, emit_receiver) = broadcast::channel(128);

    {
        let emit_sender = emit_sender.clone();
        scheduler.schedule(async move {
            while let Some(msg_result) = incoming_stream.next().await {
                match msg_result {
                    Ok(msg) => match msg.message {
                        Some(collector::control_plane_message::Message::ActionInvoked(action)) => {
                            log::info!(
                                "Action invoked: id={}, key={}, token={}",
                                action.id,
                                action.action_key,
                                action.token
                            );
                            let _ = emit_sender.send(EmitEvent::ActionInvoked(action));
                        }
                        Some(collector::control_plane_message::Message::NotificationClosed(
                            closed,
                        )) => {
                            log::info!(
                                "Notification closed by control plane: id={}, reason={:?}",
                                closed.id,
                                closed.reason()
                            );
                            let _ = emit_sender.send(EmitEvent::NotificationClosed(closed));
                        }
                        None => {
                            log::warn!("Received empty ControlPlaneMessage");
                        }
                    },
                    Err(e) => {
                        log::error!("Error receiving message from control plane: {}", e);
                        break;
                    }
                }
            }
        })?;
    }

    {
        let event_sender = event_sender.clone();
        scheduler.schedule(async move {
            if let Err(e) = dbus::serve(event_sender, emit_receiver).await {
                log::error!("{e}");
            }
        })?;
    }

    event_loop
        .handle()
        .insert_source(executor, |(), (), _: &mut Collector| ())
        .map_err(|e| anyhow::anyhow!("Failed to insert source: {e}"))?;

    event_loop
        .handle()
        .insert_source(event_receiver, |event, (), collector| {
            if let calloop::channel::Event::Msg(event) = event
                && let Err(e) = pollster::block_on(collector.handle_app_event(event))
            {
                log::error!("Failed to handle event: {e}");
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert source: {e}"))?;

    event_loop.run(None, &mut collector, |_| {})?;

    Ok(())
}
