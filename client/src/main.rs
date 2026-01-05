use wayland_backend as _;

pub mod moxnotify {
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
    pub mod client {
        tonic::include_proto!("moxnotify.client");
    }
}

mod audio;
pub mod components;
mod dbus;
mod grpc;
mod input;
mod manager;
mod rendering;
pub mod utils;
mod wayland;

use crate::utils::wait;
use audio::Audio;
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use clap::Parser;
use components::notification::NotificationId;
use config::client::ClientConfig as Config;
use config::client::keymaps;
use glyphon::FontSystem;
use input::Seat;
use manager::NotificationManager;
use moxnotify::client::{ClientActionInvokedRequest, GetViewportRequest};
use moxnotify::types::CloseReason;
use moxnotify::types::{ActionInvoked, NewNotification, Urgency};
use rendering::surface::{FocusReason, Surface};
use rendering::wgpu_state;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::broadcast;
use wayland_client::globals::{GlobalList, registry_queue_init};
use wayland_client::protocol::{wl_compositor, wl_output};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, delegate_noop};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1;

#[derive(Debug)]
pub struct Output {
    id: u32,
    name: Option<Arc<str>>,
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
    output: Option<Arc<str>>,
}

impl Moxnotify {
    async fn new(
        conn: &Connection,
        qh: QueueHandle<Moxnotify>,
        globals: GlobalList,
        loop_handle: calloop::LoopHandle<'static, Self>,
        emit_sender: broadcast::Sender<EmitEvent>,
        event_sender: calloop::channel::Sender<Event>,
        config: Arc<Config>,
    ) -> anyhow::Result<Self> {
        let layer_shell = globals.bind(&qh, 1..=5, ())?;
        let compositor = globals.bind::<wl_compositor::WlCompositor, _, _>(&qh, 1..=6, ())?;
        let seat = Seat::new(&qh, &globals)?;

        let wgpu_state = wgpu_state::WgpuState::new(conn).await?;

        let font_system = Rc::new(RefCell::new(FontSystem::new()));

        Ok(Self {
            // TODO: figure out a better way to handle it, Box clone is expensive
            output: config.general.output.clone(),
            audio: Audio::try_new().unwrap(),
            globals,
            qh,
            notifications: NotificationManager::new(
                Arc::clone(&config),
                event_sender.clone(),
                Rc::clone(&font_system),
            )
            .await,
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
                    self.dismiss_range(.., Some(CloseReason::ReasonDismissedByUser));
                } else if id == 0 {
                    if let Some(notification) = self.notifications.notifications().front() {
                        log::info!("Dismissing first notification (id={})", notification.id());
                        self.dismiss_with_reason(
                            notification.id(),
                            Some(CloseReason::ReasonDismissedByUser),
                        );
                    } else {
                        log::debug!("No notifications to dismiss");
                    }
                } else {
                    log::info!("Dismissing notification with id={id}");
                    self.dismiss_with_reason(id, Some(CloseReason::ReasonDismissedByUser));
                }
            }
            Event::InvokeAction { id, key, uuid } => {
                if let Some(surface) = self.surface.as_ref() {
                    let token = surface.token.as_ref().map(Arc::clone);

                    log::info!("Action invoked: id: {}, key: {}", id, key);

                    let mut grpc_client = self.notifications.grpc_client.clone();
                    _ = wait(move || async move {
                        grpc_client
                            .action_invoked(tonic::Request::new(ClientActionInvokedRequest {
                                action_invoked: Some(ActionInvoked {
                                    id,
                                    action_key: key,
                                    token: token.unwrap_or_default().to_string(),
                                    uuid,
                                }),
                            }))
                            .await
                            .unwrap();
                    });
                }

                if !self
                    .notifications
                    .notifications()
                    .iter()
                    .find(|notification| notification.id() == id)
                    .is_some_and(|n| n.data().hints.as_ref().unwrap().resident)
                {
                    self.dismiss_with_reason(id, Some(CloseReason::ReasonCloseNotificationCall));
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
                    data.hints.as_ref().unwrap().sound_file.clone(),
                    data.hints.as_ref().unwrap().sound_name.clone(),
                ) {
                    (None, Some(sound_name)) => freedesktop_sound::lookup(&sound_name)
                        .with_cache()
                        .find()
                        .map(std::convert::Into::into),
                    (None, None) => {
                        match Urgency::try_from(data.hints.as_ref().unwrap().urgency).unwrap() {
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
                        }
                    }
                    (Some(sound_file), Some(_) | None) => {
                        let str = sound_file.as_str();
                        PathBuf::from_str(str).map(|path| path.into()).ok()
                    }
                };

                let suppress_sound = data.hints.as_ref().unwrap().suppress_sound;

                self.notifications.add(*data);

                let mut grpc_client = self.notifications.grpc_client.clone();
                if let Ok(response) = wait(|| async move {
                    grpc_client
                        .get_viewport(tonic::Request::new(GetViewportRequest {}))
                        .await
                        .unwrap()
                        .into_inner()
                }) {
                    self.notifications.notification_view.update(
                        response.focused_ids,
                        response.before_count,
                        response.after_count,
                    );

                    if let Some(selected_id) = response.selected_id
                        && self.notifications.selected_id().is_some()
                    {
                        self.notifications.select(selected_id);
                    }
                }

                if self.notifications.inhibited() || suppress_sound {
                    log::debug!("Sound suppressed for notification");
                } else if let Some(path) = path {
                    log::debug!("Playing notification sound");
                    if let Err(e) = self.audio.play(&path) {
                        log::warn!("Failed to play audio file: {}, {e}", path.display());
                    }
                }
            }
            Event::CloseNotification(id) => {
                log::info!("Closing notification with id={id}");
                self.dismiss_with_reason(id, None);
            }
            Event::FocusSurface => {
                if let Some(surface) = self.surface.as_mut()
                    && surface.focus_reason.is_none()
                {
                    log::info!("Focusing notification surface");
                    surface.focus(FocusReason::Ctl);

                    let mut grpc_client = self.notifications.grpc_client.clone();
                    if let Ok(response) = wait(|| async move {
                        grpc_client
                            .get_viewport(tonic::Request::new(GetViewportRequest {}))
                            .await
                            .unwrap()
                            .into_inner()
                    }) && let Some(selected) = response.selected_id
                    {
                        self.notifications.select(selected);
                    } else {
                        self.notifications.first();
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
            Event::Waiting => {
                log::debug!("Getting waiting notification count");
                _ = self
                    .emit_sender
                    .send(EmitEvent::Waiting(self.notifications.waiting()));

                return Ok(());
            }
            Event::SetOutput(output) => {
                log::info!("Setting output to: {output:?}");
                self.output = output;
            }
            Event::ShowOutput => {
                log::debug!("Getting current output");
                _ = self.emit_sender.send(EmitEvent::ShowOutput(
                    self.output
                        .as_ref()
                        .map(Arc::clone)
                        .unwrap_or("auto".into()),
                ));
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

#[derive(Clone)]
pub enum EmitEvent {
    Waiting(usize),
    Open {
        uri: Arc<str>,
        token: Option<Arc<str>>,
    },
    List(Vec<String>),
    MuteStateChanged(bool),
    InhibitStateChanged(bool),
    Muted(bool),
    Inhibited(bool),
    ShowOutput(Arc<str>),
}

#[derive(Debug)]
pub enum Event {
    Waiting,
    Dismiss {
        all: bool,
        id: NotificationId,
    },
    InvokeAction {
        id: NotificationId,
        key: String,
        uuid: String,
    },
    InvokeAnchor(Arc<str>),
    Notify(Box<NewNotification>),
    CloseNotification(u32),
    List,
    FocusSurface,
    Mute,
    Unmute,
    GetMuted,
    Inhibit,
    Uninhibit,
    GetInhibited,
    SetOutput(Option<Arc<str>>),
    ShowOutput,
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
    #[arg(short, long, value_name = "FILE", help = "Path to the config file")]
    config: Option<Box<Path>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config =
        config::Config::load(cli.config.as_ref().map(|p| p.as_ref())).unwrap_or_else(|err| {
            log::warn!("{err}");
            config::Config::default()
        });
    env_logger::Builder::new()
        .filter(Some("client"), config.client.log_level.into())
        .init();

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let (emit_sender, _emit_receiver) = broadcast::channel(std::mem::size_of::<EmitEvent>());
    let (event_sender, event_receiver) = calloop::channel::channel();
    let mut event_loop = EventLoop::try_new()?;
    let mut moxnotify = Moxnotify::new(
        &conn,
        qh,
        globals,
        event_loop.handle(),
        emit_sender.clone(),
        event_sender.clone(),
        Arc::new(config.client),
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
        let client = moxnotify.notifications.grpc_client.clone();
        let max_visible = moxnotify.config.general.max_visible;
        scheduler.schedule(async move {
            if let Err(e) = grpc::serve(client, event_sender, max_visible as u32).await {
                log::error!("{:?}", e);
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
