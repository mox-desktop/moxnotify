use crate::image_data::ImageData;
use crate::moxnotify::types::{
    Action, CloseReason, Image, ImageData as ProtoImageData, NewNotification, NotificationHints,
    Urgency,
};
use crate::{EmitEvent, Event};
use chrono::offset::Local;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use zbus::{fdo::RequestNameFlags, object_server::SignalEmitter, zvariant::Str};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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

impl NotificationHints {
    fn new(hints: HashMap<&str, zbus::zvariant::Value<'_>>) -> Self {
        hints
            .into_iter()
            .fold(NotificationHints::default(), |mut nh, (k, v)| {
                match k {
                    "action-icons" => {
                        nh.action_icons = match v {
                            zbus::zvariant::Value::Bool(b) => b,
                            zbus::zvariant::Value::I32(n) => n != 0,
                            zbus::zvariant::Value::U32(n) => n != 0,
                            zbus::zvariant::Value::Str(s) => s.eq_ignore_ascii_case("true"),
                            _ => false,
                        };
                    }
                    "category" => nh.category = Str::try_from(v).ok().map(|s| s.as_str().into()),
                    "value" => nh.value = i32::try_from(v).ok(),
                    "desktop-entry" => {
                        nh.desktop_entry = Str::try_from(v).ok().map(|s| s.as_str().into());
                    }
                    "resident" => {
                        nh.resident = match v {
                            zbus::zvariant::Value::Bool(b) => b,
                            zbus::zvariant::Value::I32(n) => n != 0,
                            zbus::zvariant::Value::U32(n) => n != 0,
                            zbus::zvariant::Value::Str(s) => s.eq_ignore_ascii_case("true"),
                            _ => false,
                        };
                    }
                    "sound-file" => {
                        nh.sound_file = Str::try_from(v).ok().map(|s| s.to_string());
                    }
                    "sound-name" => {
                        nh.sound_name = Str::try_from(v).ok().map(|s| s.to_string());
                    }
                    "suppress-sound" => {
                        nh.suppress_sound = match v {
                            zbus::zvariant::Value::Bool(b) => b,
                            zbus::zvariant::Value::I32(n) => n != 0,
                            zbus::zvariant::Value::U32(n) => n != 0,
                            zbus::zvariant::Value::Str(s) => s.eq_ignore_ascii_case("true"),
                            _ => false,
                        };
                    }
                    "transient" => {
                        nh.transient = match v {
                            zbus::zvariant::Value::Bool(b) => b,
                            zbus::zvariant::Value::I32(n) => n != 0,
                            zbus::zvariant::Value::U32(n) => n != 0,
                            zbus::zvariant::Value::Str(s) => s.eq_ignore_ascii_case("true"),
                            _ => false,
                        };
                    }
                    "x" => nh.x = i32::try_from(v).unwrap_or_default(),
                    "y" => nh.y = i32::try_from(v).ok(),
                    "urgency" => {
                        nh.urgency = match u8::try_from(v) {
                            Ok(0) => Urgency::Low as i32,
                            Ok(1) => Urgency::Normal as i32,
                            Ok(2) => Urgency::Critical as i32,
                            _ => {
                                log::warn!("Invalid urgency data");
                                Urgency::Normal as i32
                            }
                        };
                    }
                    "image-path" | "image_path" => {
                        if let Ok(s) = Str::try_from(v) {
                            nh.image = if let Ok(path) = url::Url::parse(&s) {
                                Some(Image {
                                    image: Some(crate::moxnotify::types::image::Image::FilePath(
                                        path.to_string(),
                                    )),
                                })
                            } else {
                                Some(Image {
                                    image: Some(crate::moxnotify::types::image::Image::Name(
                                        s.as_str().into(),
                                    )),
                                })
                            };
                        }
                    }
                    "image-data" | "image_data" | "icon_data" => {
                        if let zbus::zvariant::Value::Structure(v) = v {
                            if let Ok(image) = ImageData::try_from(v) {
                                nh.image = Some(Image {
                                    image: Some(crate::moxnotify::types::image::Image::Data(
                                        convert_image_data(&image),
                                    )),
                                });
                            } else {
                                log::warn!("Invalid image data");
                            }
                        }
                    }
                    _ => log::warn!("Unknown hint: {k}"),
                }
                nh
            })
    }
}

struct NotificationsImpl {
    next_id: u32,
    event_sender: tokio::sync::mpsc::Sender<Event>,
    uuid: String,
    config: Arc<config::Config>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationsImpl {
    async fn get_capabilities(&self) -> &[&'static str] {
        &[
            "action-icons",
            "actions",
            "body",
            "body-hyperlinks",
            "body-images",
            "body-markup",
            "icon-multi",
            "persistence",
            "sound",
        ]
    }

    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &mut self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Box<[&str]>,
        hints: HashMap<&str, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id == 0 {
            let id = self.next_id;
            self.next_id = self.next_id.checked_add(1).unwrap_or(1);
            id
        } else {
            replaces_id
        };

        let app_icon: Option<String> = if app_icon.is_empty() {
            None
        } else {
            Some(app_icon.to_string())
        };

        let hints = NotificationHints::new(hints);
        let timeout = if expire_timeout == -1 {
            match Urgency::try_from(hints.urgency).unwrap() {
                Urgency::Low => self.config.collector.default_timeout.urgency_low * 1000,
                Urgency::Normal => self.config.collector.default_timeout.urgency_normal * 1000,
                Urgency::Critical => self.config.collector.default_timeout.urgency_critical * 1000,
            }
        } else {
            expire_timeout
        };

        if let Err(e) = self
            .event_sender
            .send(Event::Notify(Box::new(NewNotification {
                id,
                app_name: app_name.into(),
                summary: summary.into(),
                body: body.into(),
                timeout,
                actions: actions
                    .chunks_exact(2)
                    .map(|action| Action {
                        key: action[0].to_string(),
                        label: action[1].to_string(),
                    })
                    .collect(),
                hints: Some(hints),
                app_icon,
                timestamp: Local::now().timestamp_millis(),
                uuid: self.uuid.clone(),
            })))
            .await
        {
            log::error!("Error: {e}");
        }

        id
    }

    async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
        if let Err(e) = self.event_sender.send(Event::CloseNotification(id)).await {
            log::error!("Failed to send CloseNotification({id}) event: {e}");
        }

        Ok(())
    }

    async fn get_server_information(
        &self,
    ) -> zbus::fdo::Result<(&'static str, &'static str, &'static str, &'static str)> {
        Ok(("moxnotify", "mox", VERSION, "1.2"))
    }

    #[zbus(signal)]
    async fn notification_closed(
        signal_emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        signal_emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn activation_token(
        signal_emitter: &SignalEmitter<'_>,
        id: u32,
        activation_token: &str,
    ) -> zbus::Result<()>;
}

pub async fn serve(
    event_sender: tokio::sync::mpsc::Sender<Event>,
    mut emit_receiver: broadcast::Receiver<EmitEvent>,
    uuid: String,
    config: Arc<config::Config>,
) -> zbus::Result<()> {
    let server = NotificationsImpl {
        next_id: 1,
        event_sender,
        uuid: uuid.clone(),
        config,
    };

    let conn = zbus::connection::Builder::session()?
        .serve_at("/org/freedesktop/Notifications", server)?
        .build()
        .await?;

    if let Err(e) = conn
        .request_name_with_flags(
            "org.freedesktop.Notifications",
            RequestNameFlags::DoNotQueue.into(),
        )
        .await
    {
        log::error!("{e}, is another daemon running?");
        std::process::exit(0);
    }

    let iface = conn
        .object_server()
        .interface::<_, NotificationsImpl>("/org/freedesktop/Notifications")
        .await?;

    loop {
        match emit_receiver.recv().await {
            Ok(EmitEvent::ActionInvoked(action)) => {
                log::info!(
                    "{} action invoked for notification with ID: {}.",
                    action.action_key,
                    action.id
                );

                _ = NotificationsImpl::activation_token(
                    iface.signal_emitter(),
                    action.id,
                    &action.token,
                )
                .await;

                _ = NotificationsImpl::action_invoked(
                    iface.signal_emitter(),
                    action.id,
                    &action.action_key,
                )
                .await;
            }
            Ok(EmitEvent::NotificationClosed(closed)) => {
                let reason = match closed.reason() {
                    CloseReason::ReasonExpired => 1,
                    CloseReason::ReasonDismissedByUser => 2,
                    CloseReason::ReasonCloseNotificationCall => 3,
                    CloseReason::ReasonUnknown => 4,
                };

                if closed.uuid == uuid {
                    log::info!(
                        "Notification with ID: {} was closed. Reason: {:?}",
                        closed.id,
                        closed.reason()
                    );

                    _ = NotificationsImpl::notification_closed(
                        iface.signal_emitter(),
                        closed.id,
                        reason,
                    )
                    .await;
                } else {
                    log::debug!(
                        "Notification with ID: {} was closed but uuid doesn't match, ignoring.",
                        closed.id,
                    );
                }
            }
            _ => {}
        }
    }
}
