mod audio;
pub mod components;
mod config;
mod dbus;
mod input;
mod manager;
mod rendering;
pub mod utils;

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
use rusqlite::params;
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
    globals::{GlobalList, GlobalListContents, registry_queue_init},
    protocol::{wl_compositor, wl_output, wl_registry},
};
use wayland_protocols::xdg::activation::v1::client::{xdg_activation_token_v1, xdg_activation_v1};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1;
use zbus::zvariant::Type;

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

#[derive(Default, PartialEq, Clone, Copy, Type, Serialize)]
pub enum History {
    #[default]
    Hidden,
    Shown,
}

pub struct Moxnotify {
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
    db: rusqlite::Connection,
    history: History,
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

        let db = rusqlite::Connection::open(&config.general.history.path)?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS notifications (
            rowid INTEGER PRIMARY KEY AUTOINCREMENT,
            id INTEGER,
            app_name TEXT,
            app_icon TEXT,
            summary TEXT,
            body TEXT,
            timeout INTEGER,
            actions TEXT,
            hints JSON
        );",
            (),
        )?;

        let font_system = Rc::new(RefCell::new(FontSystem::new()));

        Ok(Self {
            history: History::Hidden,
            db,
            audio: Audio::new(),
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
                    if let Some(notification) = self.notifications.notifications().first() {
                        log::info!("Dismissing first notification (id={})", notification.id());
                        self.dismiss_by_id(notification.id(), Some(Reason::DismissedByUser));
                    } else {
                        log::debug!("No notifications to dismiss");
                    }
                } else {
                    log::info!("Dismissing notification with id={id}");
                    self.dismiss_by_id(id, Some(Reason::DismissedByUser));
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
                    .map(|n| n.data().hints.resident)
                    .unwrap_or_default()
                {
                    self.dismiss_by_id(id, None);
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
                    (Some(sound_file), None) => Some(sound_file),
                    (None, Some(sound_name)) => freedesktop_sound::lookup(&sound_name)
                        .with_cache()
                        .find()
                        .map(|s| s.into()),
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
                    (Some(sound_file), Some(_)) => Some(sound_file),
                };

                let suppress_sound = data.hints.suppress_sound;

                let id = match self.history {
                    History::Shown => self.db.last_insert_rowid() as u32,
                    History::Hidden => data.id,
                };

                self.notifications.add(NotificationData { id, ..*data })?;

                if self.notifications.inhibited() || suppress_sound {
                    log::debug!("Sound suppressed for notification");
                } else if let Some(path) = path {
                    log::debug!("Playing notification sound");
                    self.audio.play(path)?;
                }

                if let Some(notification) = self.notifications.notifications().last() {
                    self.db.execute(
                        "INSERT INTO notifications (id, app_name, app_icon, timeout, summary, body, actions, hints)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            notification.data().id,
                            notification.data().app_name,
                            notification.data().app_icon,
                            notification.data().timeout,
                            notification.data().summary,
                            notification.data().body,
                            serde_json::to_string(&notification.data().actions)?,
                            serde_json::to_string(&notification.data().hints)?
                        ],
                )?;
                }
            }
            Event::CloseNotification(id) => {
                log::info!("Closing notification with id={id}");
                self.dismiss_by_id(id, Some(Reason::CloseNotificationCall))
            }
            Event::FocusSurface => {
                if let Some(surface) = self.surface.as_mut() {
                    if surface.focus_reason.is_none() {
                        log::info!("Focusing notification surface");
                        surface.focus(FocusReason::Ctl);

                        let should_select_last =
                            self.notifications.notifications().iter().any(|n| {
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
                if !self.audio.muted() {
                    log::info!("Muting notification sounds");
                    _ = self.emit_sender.send(EmitEvent::MuteStateChanged(true));
                    self.audio.mute();
                } else {
                    log::debug!("Audio already muted");
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
                if self.history == History::Hidden {
                    self.db.execute(
                        "DELETE FROM notifications WHERE rowid IN (
                            SELECT rowid FROM notifications 
                            ORDER BY rowid ASC 
                            LIMIT MAX(0, (SELECT COUNT(*) FROM notifications) - ?)
                        )",
                        params![self.config.general.history.size],
                    )?;

                    log::info!("Showing notification history");
                    self.history = History::Shown;
                    _ = self
                        .emit_sender
                        .send(EmitEvent::HistoryStateChanged(self.history));
                    self.dismiss_range(.., Some(Reason::Expired));
                    let mut stmt = self.db.prepare("SELECT rowid, app_name, app_icon, summary, body, actions, hints FROM notifications ORDER BY rowid DESC")?;
                    let rows = stmt.query_map([], |row| {
                        Ok(NotificationData {
                            id: row.get(0)?,
                            app_name: row.get(1)?,
                            app_icon: row.get::<_, Option<Box<str>>>(2)?,
                            summary: row.get::<_, Box<str>>(3)?,
                            body: row.get::<_, Box<str>>(4)?,
                            timeout: 0,
                            actions: {
                                let json: Box<str> = row.get(5)?;
                                serde_json::from_str(&json).unwrap()
                            },
                            hints: {
                                let json: Box<str> = row.get(6)?;
                                serde_json::from_str(&json).unwrap()
                            },
                        })
                    })?;
                    let notifications = rows.collect::<Result<Vec<_>, _>>()?;
                    log::info!("Loaded {} historical notifications", notifications.len());
                    self.notifications.add_many(notifications)?;
                    log::debug!("History view completed");
                } else {
                    log::debug!("History already shown");
                }
            }
            Event::HideHistory => {
                if self.history == History::Shown {
                    self.db.execute(
                        "DELETE FROM notifications WHERE rowid IN (
                        SELECT rowid FROM notifications ORDER BY rowid ASC LIMIT (
                            SELECT MAX(COUNT(*) + 1 - ?, 0) FROM notifications
                        )
                    )",
                        params![self.config.general.history.size],
                    )?;

                    log::info!("Hiding notification history");
                    self.history = History::Hidden;
                    _ = self
                        .emit_sender
                        .send(EmitEvent::HistoryStateChanged(self.history));
                    self.dismiss_range(.., None);
                    log::debug!("History view dismissed");
                } else {
                    log::debug!("History already hidden");
                }
            }
            Event::Inhibit => {
                if !self.notifications.inhibited() {
                    log::info!("Inhibiting notifications");
                    self.notifications.inhibit();
                    _ = self.emit_sender.send(EmitEvent::InhibitStateChanged(
                        self.notifications.inhibited(),
                    ));
                } else {
                    log::debug!("Notifications already inhibited");
                }
            }
            Event::Uninhibit => {
                if self.notifications.inhibited() {
                    log::info!("Uninhibiting notifications");

                    let count = self.notifications.waiting();
                    log::debug!("Processing {count} waiting notifications");

                    let mut stmt = self.db.prepare("SELECT id, app_name, app_icon, summary, body, timeout, actions, hints FROM notifications ORDER BY rowid DESC LIMIT ?1")?;
                    let rows = stmt.query_map([count], |row| {
                        Ok(NotificationData {
                            id: row.get(0)?,
                            app_name: row.get(1)?,
                            app_icon: row.get::<_, Option<Box<str>>>(2)?,
                            summary: row.get::<_, Box<str>>(3)?,
                            body: row.get::<_, Box<str>>(4)?,
                            timeout: row.get(5)?,
                            actions: {
                                let json: Box<str> = row.get(6)?;
                                serde_json::from_str(&json).unwrap()
                            },
                            hints: {
                                let json: Box<str> = row.get(7)?;
                                serde_json::from_str(&json).unwrap()
                            },
                        })
                    })?;

                    self.notifications.uninhibit();
                    _ = self.emit_sender.send(EmitEvent::InhibitStateChanged(
                        self.notifications.inhibited(),
                    ));

                    rows.into_iter()
                        .try_for_each(|notification| self.notifications.add(notification?))?;
                    drop(stmt);
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
                _ = self.emit_sender.send(EmitEvent::HistoryState(self.history));

                return Ok(());
            }
            Event::Waiting => {
                log::debug!("Getting waiting notification count");
                _ = self
                    .emit_sender
                    .send(EmitEvent::Waiting(self.notifications.waiting()));

                return Ok(());
            }
        };

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
    Waiting(u32),
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
    HistoryStateChanged(History),
    InhibitStateChanged(bool),
    Muted(bool),
    HistoryState(History),
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

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Moxnotify {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                if interface.as_str() == "wl_output" {
                    let output = registry.bind::<wl_output::WlOutput, _, _>(name, version, qh, ());

                    let output = Output::new(output, name);
                    state.outputs.push(output);
                }
            }
            wl_registry::Event::GlobalRemove { name } => {
                state.outputs.retain(|output| output.id != name);
            }
            _ => unreachable!(),
        }
    }
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

impl Dispatch<xdg_activation_token_v1::XdgActivationTokenV1, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        _: &xdg_activation_token_v1::XdgActivationTokenV1,
        event: <xdg_activation_token_v1::XdgActivationTokenV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token } = event {
            if let Some(surface) = state.surface.as_mut() {
                surface.token = Some(token.into());
            }
        }
    }
}

delegate_noop!(Moxnotify: xdg_activation_v1::XdgActivationV1);
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
        list.iter().for_each(|global| {
            if global.interface == wl_output::WlOutput::interface().name {
                let wl_output = moxnotify.globals.registry().bind(
                    global.name,
                    global.version,
                    &moxnotify.qh,
                    (),
                );
                let output = Output::new(wl_output, global.name);
                moxnotify.outputs.push(output);
            }
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
        if let Err(e) = dbus::portal::serve(emit_receiver).await {
            log::error!("{e}");
        }
    })?;

    event_loop
        .handle()
        .insert_source(executor, |_: (), _, _| ())
        .map_err(|e| anyhow::anyhow!("Failed to insert source: {}", e))?;

    event_loop
        .handle()
        .insert_source(event_receiver, |event, _, moxnotify| {
            if let calloop::channel::Event::Msg(event) = event {
                if let Err(e) = moxnotify.handle_app_event(event) {
                    log::error!("Failed to handle event: {e}");
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert source: {}", e))?;

    event_loop.run(None, &mut moxnotify, |_| {})?;

    Ok(())
}
