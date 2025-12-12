mod dbus;
mod image_data;
pub mod collector {
    tonic::include_proto!("collector");
}

use crate::dbus::NotificationData;
use calloop::EventLoop;
use collector::collector_message::Message;
use collector::control_plane_client::ControlPlaneClient;
use collector::{
    Action, ActionInvoked, CloseReason, CollectorMessage, Image as ProtoImage,
    ImageData as ProtoImageData, NotificationClosed, NotificationHints as ProtoNotificationHints,
};
use env_logger::Builder;
use image_data::ImageData;
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

fn convert_image_data(image_data: &ImageData) -> ProtoImageData {
    ProtoImageData {
        width: image_data.width(),
        height: image_data.height(),
        rowstride: image_data.rowstride(),
        has_alpha: image_data.has_alpha(),
        bits_per_sample: image_data.bits_per_sample(),
        channels: image_data.channels(),
        data: image_data.data().to_vec(),
    }
}

fn convert_image(image: &dbus::Image) -> Option<ProtoImage> {
    Some(ProtoImage {
        image: Some(match image {
            dbus::Image::Name(name) => collector::image::Image::Name(name.to_string()),
            dbus::Image::File(path) => {
                collector::image::Image::FilePath(path.to_string_lossy().to_string())
            }
            dbus::Image::Data(image_data) => {
                collector::image::Image::Data(convert_image_data(image_data))
            }
        }),
    })
}

fn convert_hints(hints: &dbus::NotificationHints) -> ProtoNotificationHints {
    ProtoNotificationHints {
        action_icons: hints.action_icons,
        category: hints.category.clone(),
        value: hints.value,
        desktop_entry: hints.desktop_entry.clone(),
        resident: hints.resident,
        sound_file: hints
            .sound_file
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        sound_name: hints.sound_name.clone(),
        suppress_sound: hints.suppress_sound,
        transient: hints.transient,
        x: hints.x,
        y: hints.y,
        urgency: hints.urgency,
        image: hints.image.as_ref().and_then(convert_image),
    }
}

impl Collector {
    async fn handle_app_event(&mut self, event: Event) -> anyhow::Result<()> {
        let msg = match event {
            Event::Notify(data) => {
                let actions: Vec<Action> = data
                    .actions
                    .iter()
                    .map(|(key, label)| Action {
                        key: key.clone(),
                        label: label.clone(),
                    })
                    .collect();

                log::info!(
                    "Collected notification: id={}, app_name='{}', app_icon={:?}, summary='{}', body='{}', timeout={}, actions={:?}, hints={{ urgency={:?}, category={:?}, desktop_entry={:?}, resident={}, transient={}, suppress_sound={}, action_icons={}, x={}, y={:?}, value={:?}, sound_file={:?}, sound_name={:?}, image={:?} }}",
                    data.id,
                    data.app_name,
                    data.app_icon,
                    data.summary,
                    data.body,
                    data.timeout,
                    data.actions,
                    data.hints.urgency,
                    data.hints.category,
                    data.hints.desktop_entry,
                    data.hints.resident,
                    data.hints.transient,
                    data.hints.suppress_sound,
                    data.hints.action_icons,
                    data.hints.x,
                    data.hints.y,
                    data.hints.value,
                    data.hints.sound_file.as_ref().map(|p| p.to_string_lossy()),
                    data.hints.sound_name,
                    data.hints.image.as_ref().map(|img| match img {
                        dbus::Image::Name(name) => format!("Name({})", name),
                        dbus::Image::File(path) => format!("File({})", path.display()),
                        dbus::Image::Data(img_data) =>
                            format!("Data({}x{})", img_data.width(), img_data.height()),
                    })
                );

                CollectorMessage {
                    message: Some(Message::NewNotification(collector::NewNotification {
                        id: data.id,
                        app_name: data.app_name.clone(),
                        app_icon: data.app_icon.clone(),
                        summary: data.summary.clone(),
                        body: data.body.clone(),
                        timeout: data.timeout,
                        actions,
                        hints: Some(convert_hints(&data.hints)),
                    })),
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
    Notify(Box<NotificationData>),
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
