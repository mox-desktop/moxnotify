use crate::EmitEvent;
use crate::Event;
use crate::collector;
use crate::collector::{
    Action, CloseReason, Image, ImageData as ProtoImageData, NewNotification, NotificationHints,
};
use crate::image_data::ImageData;
use chrono::offset::Local;
#[cfg(not(debug_assertions))]
use futures_lite::stream::StreamExt;
use std::collections::HashMap;
use tokio::sync::broadcast;
#[cfg(not(debug_assertions))]
use zbus::fdo::DBusProxy;
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
                            Ok(0) => 0_i32,
                            Ok(1) => 1_i32,
                            Ok(2) => 2_i32,
                            _ => {
                                log::warn!("Invalid urgency data");
                                1_i32
                            }
                        };
                    }
                    "image-path" | "image_path" => {
                        if let Ok(s) = Str::try_from(v) {
                            nh.image = if let Ok(path) = url::Url::parse(&s) {
                                Some(Image {
                                    image: Some(collector::image::Image::FilePath(
                                        path.to_string(),
                                    )),
                                })
                            } else {
                                Some(Image {
                                    image: Some(collector::image::Image::Name(s.as_str().into())),
                                })
                            };
                        }
                    }
                    "image-data" | "image_data" | "icon_data" => {
                        if let zbus::zvariant::Value::Structure(v) = v {
                            if let Ok(image) = ImageData::try_from(v) {
                                nh.image = Some(Image {
                                    image: Some(collector::image::Image::Data(convert_image_data(
                                        &image,
                                    ))),
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
    event_sender: calloop::channel::Sender<Event>,
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

        if let Err(e) = self
            .event_sender
            .send(Event::Notify(Box::new(NewNotification {
                id,
                app_name: app_name.into(),
                summary: summary.into(),
                body: body.into(),
                timeout: expire_timeout,
                actions: actions
                    .chunks_exact(2)
                    .map(|action| Action {
                        key: action[0].to_string(),
                        label: action[1].to_string(),
                    })
                    .collect(),
                hints: Some(NotificationHints::new(hints)),
                app_icon,
                timestamp: Local::now().timestamp_millis(),
            })))
        {
            log::error!("Error: {e}");
        }

        id
    }

    async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
        if let Err(e) = self.event_sender.send(Event::CloseNotification(id)) {
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
    event_sender: calloop::channel::Sender<Event>,
    mut emit_receiver: broadcast::Receiver<EmitEvent>,
) -> zbus::Result<()> {
    let server = NotificationsImpl {
        next_id: 1,
        event_sender,
    };

    let conn = zbus::connection::Builder::session()?
        .serve_at("/org/freedesktop/Notifications", server)?
        .build()
        .await?;

    if let Err(e) = conn
        .request_name_with_flags(
            "org.freedesktop.Notifications",
            // If in release mode, exit if well-known name is already taken
            #[cfg(not(debug_assertions))]
            (RequestNameFlags::DoNotQueue | RequestNameFlags::AllowReplacement),
            // If in debug profile, replace already existing daemon
            #[cfg(debug_assertions)]
            RequestNameFlags::ReplaceExisting.into(),
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

    #[cfg(not(debug_assertions))]
    let acquired_stream = DBusProxy::new(&conn).await?.receive_name_lost().await?;
    #[cfg(not(debug_assertions))]
    tokio::spawn(async move {
        let mut acquired_stream = acquired_stream;
        if acquired_stream.next().await.is_some() {
            log::info!("Request to ReplaceExisting on org.freedesktop.Notification received");
            std::process::exit(0);
        }
    });

    tokio::spawn(async move {
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
                }
                _ => {}
            }
        }
    });

    Ok(())
}
