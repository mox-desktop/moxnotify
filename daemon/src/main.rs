mod audio;
pub mod components;
mod config;
mod dbus;
pub mod history;
mod input;
mod manager;
mod rendering;
pub mod utils;
mod wayland;

use crate::config::keymaps;
use audio::Audio;
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use clap::Parser;
use components::notification::NotificationId;
use config::Config;
use dbus::xdg::NotificationData;
use env_logger::Builder;
use glyphon::FontSystem;
use input::Seat;
use log::LevelFilter;
use manager::{NotificationManager, Reason};
use rendering::{
    surface::{FocusReason, Surface},
    wgpu_state,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{Arc, atomic::Ordering},
};
use tokio::sync::broadcast;
use utils::image_data::ImageData;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    globals::{GlobalList, registry_queue_init},
    protocol::{wl_compositor, wl_output},
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1;

#[derive(Debug)]
pub struct Output {
    id: u32,
    name: Option<Box<str>>,
    scale: f32,
    wl_output: wl_output::WlOutput,
}

impl Output {
    fn new(wl_output: wl_output::WlOutput, id: NotificationId) -> Self {
        Self {
            id,
            name: None,
            scale: 1.0,
            wl_output,
        }
    }
}

pub struct Moxnotify {
    idle_notification: Option<ext_idle_notification_v1::ExtIdleNotificationV1>,
    layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    seat: Seat,
    surface: Option<Surface>,
    outputs: Vec<Output>,
    wgpu_state: wgpu_state::WgpuState,
    notifications: NotificationManager,
    config: Arc<Config>,
    qh: QueueHandle<Self>,
    globals: GlobalList,
    loop_handle: calloop::LoopHandle<'static, Self>,
    emit_sender: broadcast::Sender<EmitEvent>,
    compositor: wl_compositor::WlCompositor,
    audio: Audio,
    font_system: Rc<RefCell<FontSystem>>,
}

impl Moxnotify {
    async fn new<T>(
        conn: &Connection,
        qh: QueueHandle<Moxnotify>,
        globals: GlobalList,
        loop_handle: calloop::LoopHandle<'static, Self>,
        emit_sender: broadcast::Sender<EmitEvent>,
        event_sender: calloop::channel::Sender<Event>,
        config_path: Option<T>,
    ) -> anyhow::Result<Self>
    where
        T: AsRef<Path>,
    {
        let layer_shell = globals.bind(&qh, 1..=5, ())?;
        let compositor = globals.bind::<wl_compositor::WlCompositor, _, _>(&qh, 1..=6, ())?;
        let seat = Seat::new(&qh, &globals)?;

        let config = Arc::new(Config::load(config_path)?);

        let wgpu_state = wgpu_state::WgpuState::new(conn).await?;

        let font_system = Rc::new(RefCell::new(FontSystem::new()));

        let idle_notifier: Option<ext_idle_notifier_v1::ExtIdleNotifierV1> =
            globals.bind(&qh, 1..=1, ()).ok();
        let idle_notification = idle_notifier.as_ref().map(|idle_notifier| {
            idle_notifier.get_idle_notification(5 * 60 * 1000, &seat.wl_seat, &qh, ())
        });

        Ok(Self {
            idle_notification,
            audio: Audio::try_new().unwrap(),
            globals,
            qh,
            notifications: NotificationManager::new(
                Arc::clone(&config),
                loop_handle.clone(),
                event_sender.clone(),
                Rc::clone(&font_system),
            ),
            font_system,
            config,
            wgpu_state,
            layer_shell,
            seat,
            surface: None,
            outputs: Vec::new(),
            loop_handle,
            emit_sender,
            compositor,
        })
    }

    fn handle_app_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::Dismiss { all, id } => {
                if all {
                    log::info!("Dismissing all notifications");
                    self.dismiss_range(.., Some(Reason::DismissedByUser));
                } else if id == 0 {
                    if let Some(notification) = self.notifications.notifications().front() {
                        log::info!("Dismissing first notification (id={})", notification.id());
                        self.dismiss_with_reason(notification.id(), Some(Reason::DismissedByUser));
                    } else {
                        log::debug!("No notifications to dismiss");
                    }
                } else {
                    log::info!("Dismissing notification with id={id}");
                    self.dismiss_with_reason(id, Some(Reason::DismissedByUser));
                }
            }
            Event::InvokeAction { id, key } => {
                if let Some(surface) = self.surface.as_ref() {
                    let token = surface.token.as_ref().map(Arc::clone);
                    _ = self.emit_sender.send(crate::EmitEvent::ActionInvoked {
                        id,
                        key,
                        token: token.unwrap_or_default(),
                    });
                }

                if !self
                    .notifications
                    .notifications()
                    .iter()
                    .find(|notification| notification.id() == id)
                    .is_some_and(|n| n.data().hints.resident)
                {
                    self.dismiss_with_reason(id, None);
                }
            }
            Event::InvokeAnchor(uri) => {
                if let Some(surface) = self.surface.as_ref() {
                    let token = surface.token.as_ref().map(Arc::clone);
                    if self
                        .emit_sender
                        .send(EmitEvent::Open {
                            uri: Arc::clone(&uri),
                            token,
                        })
                        .is_ok()
                        && surface.focus_reason == Some(FocusReason::MouseEnter)
                    {
                        self.notifications.deselect();
                        self.notifications
                            .ui_state
                            .mode
                            .store(keymaps::Mode::Normal, Ordering::Relaxed);
                    }
                }
            }
            Event::Notify(data) => {
                log::info!(
                    "Receiving notification from {}: '{}'",
                    data.app_name,
                    data.summary
                );

                let path = match (
                    data.hints.sound_file.as_ref().map(Arc::clone),
                    data.hints.sound_name.as_ref().map(Arc::clone),
                ) {
                    (None, Some(sound_name)) => freedesktop_sound::lookup(&sound_name)
                        .with_cache()
                        .find()
                        .map(std::convert::Into::into),
                    (None, None) => match data.hints.urgency {
                        Urgency::Low => self
                            .config
                            .general
                            .default_sound_file
                            .urgency_low
                            .as_ref()
                            .map(Arc::clone),
                        Urgency::Normal => self
                            .config
                            .general
                            .default_sound_file
                            .urgency_normal
                            .as_ref()
                            .map(Arc::clone),
                        Urgency::Critical => self
                            .config
                            .general
                            .default_sound_file
                            .urgency_critical
                            .as_ref()
                            .map(Arc::clone),
                    },
                    (Some(sound_file), Some(_) | None) => Some(sound_file),
                };

                let suppress_sound = data.hints.suppress_sound;

                let id = match self.notifications.history.state() {
                    history::HistoryState::Shown => self.notifications.history.last_insert_rowid(),
                    history::HistoryState::Hidden => data.id,
                };

                self.notifications.add(NotificationData { id, ..*data });

                if self.notifications.inhibited() || suppress_sound {
                    log::debug!("Sound suppressed for notification");
                } else if let Some(path) = path {
                    log::debug!("Playing notification sound");
                    self.audio.play(path)?;
                }

                if let Some(notification) = self.notifications.notifications().back()
                    && let Err(e) = self.notifications.history.insert(notification.data())
                {
                    log::warn!("{e}");
                }
            }
            Event::CloseNotification(id) => {
                log::info!("Closing notification with id={id}");
                self.dismiss_with_reason(id, Some(Reason::CloseNotificationCall));
            }
            Event::FocusSurface => {
                if let Some(surface) = self.surface.as_mut()
                    && surface.focus_reason.is_none()
                {
                    log::info!("Focusing notification surface");
                    surface.focus(FocusReason::Ctl);

                    let should_select_last = self.notifications.notifications().iter().any(|n| {
                        n.id()
                            == self
                                .notifications
                                .ui_state
                                .selected_id
                                .load(Ordering::Relaxed)
                    });

                    if should_select_last {
                        let last_id = self
                            .notifications
                            .ui_state
                            .selected_id
                            .load(Ordering::Relaxed);
                        self.notifications.select(last_id);
                    } else {
                        self.notifications.next();
                    }
                }
            }
            Event::List => {
                log::info!("Listing all active notifications");
                let list = self
                    .notifications
                    .notifications()
                    .iter()
                    .map(|notification| serde_json::to_string(&notification.data()).unwrap())
                    .collect::<Vec<_>>();
                _ = self.emit_sender.send(EmitEvent::List(list));

                return Ok(());
            }
            Event::Mute => {
                if self.audio.muted() {
                    log::debug!("Audio already muted");
                } else {
                    log::info!("Muting notification sounds");
                    _ = self.emit_sender.send(EmitEvent::MuteStateChanged(true));
                    self.audio.mute();
                }

                return Ok(());
            }
            Event::Unmute => {
                if self.audio.muted() {
                    log::info!("Unmuting notification sounds");
                    self.audio.unmute();
                    _ = self
                        .emit_sender
                        .send(EmitEvent::MuteStateChanged(self.audio.muted()));
                } else {
                    log::debug!("Audio already unmuted");
                }

                return Ok(());
            }
            Event::ShowHistory => {
                if self.notifications.history.is_hidden() {
                    if let Err(e) = self
                        .notifications
                        .history
                        .trim(self.config.general.history.size)
                    {
                        log::warn!("{e}");
                    }

                    log::info!("Showing notification history");
                    self.notifications.history.show();
                    _ = self.emit_sender.send(EmitEvent::HistoryStateChanged(
                        self.notifications.history.state(),
                    ));
                    self.dismiss_range(.., Some(Reason::Expired));

                    let notifications = self.notifications.history.load_all()?;

                    log::info!("Loaded {} historical notifications", notifications.len());
                    self.notifications.add_many(notifications);
                    log::debug!("History view completed");
                } else {
                    log::debug!("History already shown");
                }
            }
            Event::HideHistory => {
                if self.notifications.history.is_shown() {
                    log::info!("Hiding notification history");
                    self.notifications.history.hide();
                    _ = self.emit_sender.send(EmitEvent::HistoryStateChanged(
                        self.notifications.history.state(),
                    ));
                    self.dismiss_range(.., None);
                    log::debug!("History hidden");
                } else {
                    log::debug!("History already hidden");
                }
            }
            Event::Inhibit => {
                if self.notifications.inhibited() {
                    log::debug!("Notifications already inhibited");
                } else {
                    log::info!("Inhibiting notifications");
                    self.notifications.inhibit();
                    _ = self.emit_sender.send(EmitEvent::InhibitStateChanged(
                        self.notifications.inhibited(),
                    ));
                }
            }
            Event::Uninhibit => {
                if self.notifications.inhibited() {
                    log::info!("Uninhibiting notifications");

                    let count = self.notifications.waiting();
                    log::debug!("Processing {count} waiting notifications");

                    _ = self.emit_sender.send(EmitEvent::InhibitStateChanged(
                        self.notifications.inhibited(),
                    ));
                    self.notifications.uninhibit();
                } else {
                    log::debug!("Notifications already uninhibited");
                }
            }
            Event::GetMuted => {
                log::debug!("Getting audio mute state");
                _ = self.emit_sender.send(EmitEvent::Muted(self.audio.muted()));

                return Ok(());
            }
            Event::GetInhibited => {
                log::debug!("Getting inhibit state");
                _ = self
                    .emit_sender
                    .send(EmitEvent::Inhibited(self.notifications.inhibited()));

                return Ok(());
            }
            Event::GetHistory => {
                log::debug!("Getting history state");
                _ = self
                    .emit_sender
                    .send(EmitEvent::HistoryState(self.notifications.history.state()));

                return Ok(());
            }
            Event::Waiting => {
                log::debug!("Getting waiting notification count");
                _ = self
                    .emit_sender
                    .send(EmitEvent::Waiting(self.notifications.waiting()));

                return Ok(());
            }
        }

        self.update_surface_size();
        if let Some(surface) = self.surface.as_mut() {
            surface.render(
                &self.wgpu_state.device,
                &self.wgpu_state.queue,
                &self.notifications,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Image {
    Name(Box<str>),
    File(Box<Path>),
    Data(ImageData),
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Default, Debug, Clone, Copy)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Hint {
    Value(i32),
    ActionIcons(bool),
    Category(Box<str>),
    DesktopEntry(Box<str>),
    Image(Image),
    IconData(Vec<u8>),
    Resident(bool),
    SoundFile(Box<Path>),
    SoundName(Box<str>),
    SuppressSound(bool),
    Transient(bool),
    Urgency(Urgency),
    X(i32),
    Y(i32),
}

#[derive(Clone)]
pub enum EmitEvent {
    Waiting(usize),
    ActionInvoked {
        id: NotificationId,
        key: Arc<str>,
        token: Arc<str>,
    },
    NotificationClosed {
        id: NotificationId,
        reason: Reason,
    },
    Open {
        uri: Arc<str>,
        token: Option<Arc<str>>,
    },
    List(Vec<String>),
    MuteStateChanged(bool),
    HistoryStateChanged(history::HistoryState),
    InhibitStateChanged(bool),
    Muted(bool),
    HistoryState(history::HistoryState),
    Inhibited(bool),
}

#[derive(Debug)]
pub enum Event {
    Waiting,
    Dismiss { all: bool, id: NotificationId },
    InvokeAction { id: NotificationId, key: Arc<str> },
    InvokeAnchor(Arc<str>),
    Notify(Box<NotificationData>),
    CloseNotification(u32),
    List,
    FocusSurface,
    Mute,
    Unmute,
    GetMuted,
    ShowHistory,
    HideHistory,
    GetHistory,
    Inhibit,
    Uninhibit,
    GetInhibited,
}

impl Dispatch<wl_output::WlOutput, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        wl_output: &wl_output::WlOutput,
        event: <wl_output::WlOutput as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(output) = state
            .outputs
            .iter_mut()
            .find(|output| output.wl_output == *wl_output)
        else {
            return;
        };

        match event {
            wl_output::Event::Scale { factor } => output.scale = factor as f32,
            wl_output::Event::Name { name } => output.name = Some(name.into()),
            _ => {}
        }
    }
}

delegate_noop!(Moxnotify: wl_compositor::WlCompositor);
delegate_noop!(Moxnotify: zwlr_layer_shell_v1::ZwlrLayerShellV1);

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[arg(short, long, action = clap::ArgAction::Count)]
    quiet: u8,

    #[arg(short, long, value_name = "FILE", help = "Path to the config file")]
    config: Option<Box<Path>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut log_level = LevelFilter::Info;

    (0..cli.verbose).for_each(|_| {
        log_level = match log_level {
            LevelFilter::Error => LevelFilter::Warn,
            LevelFilter::Warn => LevelFilter::Info,
            LevelFilter::Info => LevelFilter::Debug,
            LevelFilter::Debug => LevelFilter::Trace,
            _ => log_level,
        };
    });

    (0..cli.quiet).for_each(|_| {
        log_level = match log_level {
            LevelFilter::Warn => LevelFilter::Error,
            LevelFilter::Info => LevelFilter::Warn,
            LevelFilter::Debug => LevelFilter::Info,
            LevelFilter::Trace => LevelFilter::Debug,
            _ => log_level,
        };
    });

    Builder::new().filter(Some("daemon"), log_level).init();

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let (emit_sender, emit_receiver) = broadcast::channel(std::mem::size_of::<EmitEvent>());
    let (event_sender, event_receiver) = calloop::channel::channel();
    let mut event_loop = EventLoop::try_new()?;
    let mut moxnotify = Moxnotify::new(
        &conn,
        qh,
        globals,
        event_loop.handle(),
        emit_sender.clone(),
        event_sender.clone(),
        cli.config,
    )
    .await?;

    WaylandSource::new(conn, event_queue)
        .insert(event_loop.handle())
        .map_err(|e| anyhow::anyhow!("Failed to insert Wayland source: {}", e))?;

    moxnotify.globals.contents().with_list(|list| {
        list.iter()
            .filter(|global| global.interface == wl_output::WlOutput::interface().name)
            .for_each(|global| {
                let wl_output = moxnotify.globals.registry().bind(
                    global.name,
                    global.version,
                    &moxnotify.qh,
                    (),
                );
                let output = Output::new(wl_output, global.name);
                moxnotify.outputs.push(output);
            });
    });

    let (executor, scheduler) = calloop::futures::executor()?;

    {
        let event_sender = event_sender.clone();
        scheduler.schedule(async move {
            if let Err(e) = dbus::xdg::serve(event_sender, emit_receiver).await {
                log::error!("{e}");
            }
        })?;
    }

    let emit_receiver = emit_sender.subscribe();
    scheduler.schedule(async move {
        if let Err(e) = dbus::moxnotify::serve(event_sender, emit_receiver).await {
            log::error!("{e}");
        }
    })?;

    let emit_receiver = emit_sender.subscribe();
    scheduler.schedule(async move {
        if let Err(e) = dbus::portal::open_uri::serve(emit_receiver).await {
            log::error!("{e}");
        }
    })?;

    event_loop
        .handle()
        .insert_source(executor, |(), (), _| ())
        .map_err(|e| anyhow::anyhow!("Failed to insert source: {e}"))?;

    event_loop
        .handle()
        .insert_source(event_receiver, |event, (), moxnotify| {
            if let calloop::channel::Event::Msg(event) = event
                && let Err(e) = moxnotify.handle_app_event(event)
            {
                log::error!("Failed to handle event: {e}");
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert source: {e}"))?;

    event_loop.run(None, &mut moxnotify, |_| {})?;

    Ok(())
}
