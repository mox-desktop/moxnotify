mod view;

use crate::CloseReason;
use crate::{
    Moxnotify,
    components::{
        Component, Data,
        notification::{self, Notification, NotificationId},
    },
    config::{Config, keymaps},
    moxnotify::{
        client::{
            ClientNotificationClosedRequest, GetViewportRequest, RestartTimersRequest,
            StopTimersRequest, ViewportNavigationRequest,
            client_service_client::ClientServiceClient, viewport_navigation_request::Direction,
        },
        types::{NewNotification, NotificationClosed},
    },
    utils::wait,
};
use atomic_float::AtomicF32;
use glyphon::{FontSystem, TextArea};
use moxui::{shape_renderer, texture_renderer::TextureArea};
use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt,
    ops::RangeBounds,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};
use tonic::transport::Channel;
use view::NotificationView;

#[derive(Clone)]
pub struct UiState {
    pub scale: Arc<AtomicF32>,
    pub mode: Arc<keymaps::AtomicMode>,
    pub selected: Arc<AtomicBool>,
    pub selected_id: Arc<AtomicU32>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            mode: Arc::new(keymaps::AtomicMode::default()),
            scale: Arc::new(AtomicF32::new(1.0)),
            selected: Arc::new(AtomicBool::new(false)),
            selected_id: Arc::new(AtomicU32::new(0)),
        }
    }
}

pub struct NotificationManager {
    notifications: VecDeque<Notification>,
    waiting: Vec<NewNotification>,
    config: Arc<Config>,
    sender: calloop::channel::Sender<crate::Event>,
    inhibited: bool,
    font_system: Rc<RefCell<FontSystem>>,
    pub grpc_client: ClientServiceClient<Channel>,
    pub notification_view: NotificationView,
    pub ui_state: UiState,
}

impl NotificationManager {
    pub async fn new(
        config: Arc<Config>,
        sender: calloop::channel::Sender<crate::Event>,
        font_system: Rc<RefCell<FontSystem>>,
    ) -> Self {
        let scheduler_addr = "http://[::1]:64202".to_string();

        log::info!("Connecting to scheduler at: {}", scheduler_addr);

        let client = ClientServiceClient::connect(scheduler_addr).await.unwrap();

        let ui_state = UiState::default();

        Self {
            grpc_client: client,
            sender,
            inhibited: false,
            waiting: Vec::new(),
            notification_view: NotificationView::new(
                Arc::clone(&config),
                ui_state.clone(),
                Rc::clone(&font_system),
            ),
            font_system,
            notifications: VecDeque::new(),
            config,
            ui_state,
        }
    }

    /// Inhibit notifications
    pub fn inhibit(&mut self) {
        self.inhibited = true;
    }

    /// Stop inhibiting notifications and bring any inhibited
    /// notifications to the view
    pub fn uninhibit(&mut self) {
        let drained: Vec<_> = self.waiting.drain(..).collect();
        self.add_many(drained);
        self.inhibited = false;
    }

    pub fn inhibited(&mut self) -> bool {
        self.inhibited
    }

    pub fn notifications(&self) -> &VecDeque<Notification> {
        &self.notifications
    }

    pub fn data(
        &self,
    ) -> (
        Vec<shape_renderer::ShapeInstance>,
        Vec<TextArea<'_>>,
        Vec<TextureArea<'_>>,
    ) {
        let mut instances = Vec::new();
        let mut text_areas = Vec::new();
        let mut textures = Vec::new();

        self.iter_viewed()
            .flat_map(|notification| notification.get_data(notification.urgency()))
            .for_each(|data_item| match data_item {
                Data::Instance(instance) => instances.push(instance),
                Data::TextArea(text_area) => text_areas.push(text_area),
                Data::Texture(texture) => textures.push(texture),
            });

        let total_width = self
            .iter_viewed()
            .map(|notification| {
                notification.get_render_bounds().x + notification.get_render_bounds().width
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or_default();

        if let Some((instance, text_area)) = self.notification_view.prev_data(total_width) {
            instances.push(instance);
            text_areas.push(text_area);
        }

        if let Some((instance, text_area)) = self.notification_view.next_data(total_width) {
            instances.push(instance);
            text_areas.push(text_area);
        }

        (instances, text_areas, textures)
    }

    pub fn get_by_coordinates(&self, x: f64, y: f64) -> Option<&Notification> {
        self.iter_viewed().find(|notification| {
            let bounds = notification.get_render_bounds();
            x >= bounds.x as f64
                && x <= (bounds.x + bounds.width) as f64
                && y >= bounds.y as f64
                && y <= (bounds.y + bounds.height) as f64
        })
    }

    pub fn click(&mut self, x: f64, y: f64) -> bool {
        self.iter_viewed_mut().any(|notification| {
            notification
                .buttons_mut()
                .as_mut()
                .is_some_and(|buttons| buttons.click(x, y))
        })
    }

    pub fn hover(&mut self, x: f64, y: f64) -> bool {
        self.iter_viewed_mut().any(|notification| {
            notification
                .buttons_mut()
                .is_some_and(|buttons| buttons.hover(x, y))
        })
    }

    pub fn height(&self) -> f32 {
        let prev_height = self
            .notification_view
            .prev_bounds()
            .map(|b| b.height)
            .unwrap_or(0.0);

        let notification_height = self
            .iter_viewed()
            .map(|notification| notification.get_bounds().height)
            .sum::<f32>();

        let next_height = self
            .notification_view
            .next_bounds()
            .map(|b| b.height)
            .unwrap_or(0.0);

        prev_height + notification_height + next_height
    }

    pub fn width(&self) -> f32 {
        let (min_x, max_x) =
            self.iter_viewed()
                .fold((f32::MAX, f32::MIN), |(min_x, max_x), notification| {
                    let extents = notification.get_bounds();
                    let left = extents.x + notification.data().hints.as_ref().unwrap().x as f32;
                    let right = extents.x
                        + extents.width
                        + notification.data().hints.as_ref().unwrap().x as f32;
                    (min_x.min(left), max_x.max(right))
                });

        if min_x == f32::MAX || max_x == f32::MIN {
            0.0
        } else {
            max_x - min_x
        }
    }

    /// Returns the ID of the currently selected notification, if any.
    pub fn selected_id(&self) -> Option<NotificationId> {
        if self.ui_state.selected.load(Ordering::Relaxed) {
            Some(self.ui_state.selected_id.load(Ordering::Relaxed))
        } else {
            None
        }
    }

    /// Get mutable reference to the current selected notification
    pub fn selected_notification_mut(&mut self) -> Option<&mut Notification> {
        let id = self.selected_id();
        self.notifications
            .iter_mut()
            .find(|notification| Some(notification.id()) == id)
    }

    pub fn select(&mut self, id: NotificationId) {
        let Some(new_index) = self.notifications.iter().position(|n| n.id() == id) else {
            return;
        };

        let Some(notification) = self.notifications.get_mut(new_index) else {
            return;
        };

        notification.hover();
        log::info!("Selected notification id: {id}");

        self.ui_state.selected_id.store(id, Ordering::Relaxed);
        self.ui_state.selected.store(true, Ordering::Relaxed);

        let mut grpc_client = self.grpc_client.clone();
        _ = wait(|| async move {
            grpc_client
                .stop_timers(tonic::Request::new(StopTimersRequest {}))
                .await
                .unwrap();
        });

        self.update_size();
    }

    /// Deselect notification and start expiration timers
    pub fn deselect(&mut self) {
        if !self.ui_state.selected.load(Ordering::Relaxed) {
            return;
        }

        self.ui_state.selected.store(false, Ordering::Relaxed);

        if let Some(notification) = self.selected_notification_mut() {
            notification.unhover();
        }

        self.start_timers_for_visible();
    }

    pub fn start_timers_for_visible(&mut self) {
        let mut grpc_client = self.grpc_client.clone();
        _ = wait(|| async move {
            grpc_client
                .restart_timers(tonic::Request::new(RestartTimersRequest {}))
                .await
                .unwrap();
        });
    }

    /// Select next notification
    pub fn next(&mut self) {
        let mut grpc_client = self.grpc_client.clone();
        if let Ok(response) = wait(|| async move {
            grpc_client
                .navigate_viewport(tonic::Request::new(ViewportNavigationRequest {
                    direction: Direction::Next as i32,
                }))
                .await
                .unwrap()
                .into_inner()
        }) {
            if let Some(selected_id) = response.selected_id {
                self.select(selected_id);
            }

            self.notification_view.update(
                response.focused_ids,
                response.before_count,
                response.after_count,
            );
        }
    }

    /// Select previous notification
    pub fn prev(&mut self) {
        let mut grpc_client = self.grpc_client.clone();

        if let Ok(response) = wait(|| async move {
            grpc_client
                .navigate_viewport(tonic::Request::new(ViewportNavigationRequest {
                    direction: Direction::Prev as i32,
                }))
                .await
                .unwrap()
                .into_inner()
        }) {
            if let Some(selected_id) = response.selected_id {
                self.select(selected_id);
            }

            self.notification_view.update(
                response.focused_ids,
                response.before_count,
                response.after_count,
            );
        }
    }

    pub fn first(&mut self) {
        let mut grpc_client = self.grpc_client.clone();

        if let Ok(response) = wait(|| async move {
            grpc_client
                .navigate_viewport(tonic::Request::new(ViewportNavigationRequest {
                    direction: Direction::First as i32,
                }))
                .await
                .unwrap()
                .into_inner()
        }) {
            if let Some(selected_id) = response.selected_id {
                self.select(selected_id);
            }

            self.notification_view.update(
                response.focused_ids,
                response.before_count,
                response.after_count,
            );
        }
    }

    pub fn last(&mut self) {
        let mut grpc_client = self.grpc_client.clone();

        if let Ok(response) = wait(|| async move {
            grpc_client
                .navigate_viewport(tonic::Request::new(ViewportNavigationRequest {
                    direction: Direction::Last as i32,
                }))
                .await
                .unwrap()
                .into_inner()
        }) {
            if let Some(selected_id) = response.selected_id {
                self.select(selected_id);
            }

            self.notification_view.update(
                response.focused_ids,
                response.before_count,
                response.after_count,
            );
        }
    }

    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    pub fn add_many(&mut self, data: Vec<NewNotification>) {
        let new_notifications: Vec<Notification> = {
            let mut font_system = self.font_system.borrow_mut();
            data.into_iter()
                .map(|data| {
                    Notification::new(
                        Arc::clone(&self.config),
                        &mut font_system,
                        data,
                        self.ui_state.clone(),
                        Some(self.sender.clone()),
                    )
                })
                .collect()
        };

        self.notifications.extend(new_notifications);

        self.update_size();
    }

    pub fn add(&mut self, data: NewNotification) {
        if self.inhibited() {
            self.waiting.push(data);
            return;
        }

        if let Some(notification) = self.notifications.iter_mut().find(|n| n.id() == data.id) {
            notification.replace(
                &mut self.font_system.borrow_mut(),
                data,
                Some(self.sender.clone()),
            );
        } else {
            let notification = Notification::new(
                Arc::clone(&self.config),
                &mut self.font_system.borrow_mut(),
                data,
                self.ui_state.clone(),
                Some(self.sender.clone()),
            );

            self.notifications.push_back(notification);
        }

        self.update_size();
    }

    pub fn dismiss_by_id(&mut self, id: NotificationId) -> Option<Notification> {
        let index = self.notifications.iter().position(|n| n.id() == id)?;

        let notification = self.notifications.remove(index);

        if self.notifications.is_empty() {
            self.deselect();
        }

        notification
    }

    /// Returns an iterator over notifications in view
    pub fn iter_viewed(&self) -> impl Iterator<Item = &Notification> {
        self.notifications
            .iter()
            .filter(|notification| self.notification_view.visible.contains(&notification.id()))
    }

    /// Returns an iterator over notifications in view that returns mutable references
    pub fn iter_viewed_mut(&mut self) -> impl Iterator<Item = &mut Notification> {
        self.notifications.iter_mut().filter_map(|notification| {
            if self.notification_view.visible.contains(&notification.id()) {
                Some(notification)
            } else {
                None
            }
        })
    }

    pub fn update_size(&mut self) {
        let x_offset = self
            .iter_viewed()
            .map(|notification| notification.data().hints.as_ref().unwrap().x)
            .min()
            .unwrap_or_default()
            .abs() as f32;

        let mut start = self
            .notification_view
            .prev_bounds()
            .map(|bounds| bounds.y + bounds.height)
            .unwrap_or(0.0);

        self.iter_viewed_mut().for_each(|notification| {
            notification.set_position(x_offset, start);
            start += notification.get_bounds().height;
        });

        self.notification_view.set_next_position(0., start);
    }
}

impl fmt::Display for CloseReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CloseReason::ReasonExpired => "Expired",
            CloseReason::ReasonDismissedByUser => "DismissedByUser",
            CloseReason::ReasonCloseNotificationCall => "CloseNotificationCall",
            CloseReason::ReasonUnknown => "Unknown",
        };
        write!(f, "{s}")
    }
}

impl Moxnotify {
    pub fn dismiss_range<T>(&mut self, range: T, reason: Option<CloseReason>)
    where
        T: RangeBounds<usize>,
    {
        let ids: Vec<_> = self
            .notifications
            .notifications()
            .range(range)
            .map(notification::Notification::id)
            .collect();

        if let Some(reason) = reason {
            // TODO: this probably could be optimized by doing it all in the async closure
            // and lowering the amount of clones
            for id in ids.iter() {
                let uuid = self.notifications.iter_viewed().find_map(|notification| {
                    if notification.id() == *id {
                        Some(notification.uuid())
                    } else {
                        None
                    }
                });

                log::info!("Notification dismissed: id: {}, reason: {}", id, reason);
                let mut grpc_client = self.notifications.grpc_client.clone();

                let id = *id;
                _ = wait(move || async move {
                    grpc_client
                        .notification_closed(tonic::Request::new(ClientNotificationClosedRequest {
                            notification_closed: Some(NotificationClosed {
                                id,
                                reason: reason as i32,
                                uuid: uuid.unwrap(),
                            }),
                        }))
                        .await
                        .unwrap()
                });
            }
        }

        if ids.len() == self.notifications.notifications.len() {
            self.notifications.notifications.clear();
            self.notifications.update_size();
            return;
        }

        ids.iter()
            .for_each(|id| _ = self.notifications.dismiss_by_id(*id));
    }

    pub fn dismiss_with_reason(&mut self, id: u32, reason: Option<CloseReason>) {
        if self.notifications.selected_id() == Some(id) {
            self.notifications
                .ui_state
                .mode
                .store(keymaps::Mode::Normal, Ordering::Relaxed);
        }

        if let Some(notification) = self.notifications.dismiss_by_id(id) {
            let uuid = notification.uuid();

            let mut grpc_client = self.notifications.grpc_client.clone();

            let Ok(response) = wait(move || async move {
                if let Some(reason) = reason {
                    grpc_client
                        .notification_closed(tonic::Request::new(ClientNotificationClosedRequest {
                            notification_closed: Some(NotificationClosed {
                                id,
                                reason: reason as i32,
                                uuid,
                            }),
                        }))
                        .await
                        .unwrap();
                }

                grpc_client
                    .get_viewport(tonic::Request::new(GetViewportRequest {}))
                    .await
                    .unwrap()
                    .into_inner()
            }) else {
                return;
            };

            if let Some(selected_id) = response.selected_id.as_ref()
                && self.notifications.selected_id().is_some()
            {
                self.notifications.select(*selected_id);
            }

            self.notifications.notification_view.update(
                response.focused_ids,
                response.before_count,
                response.after_count,
            );

            self.update_surface_size();
            if let Some(surface) = self.surface.as_mut()
                && let Err(e) = surface.render(
                    &self.wgpu_state.device,
                    &self.wgpu_state.queue,
                    &self.notifications,
                )
            {
                log::error!("Render error: {e}");
            }

            if self.notifications.notifications().is_empty() {
                self.seat.keyboard.repeat.key = None;
            }

            log::debug!("Successfully dismissed notification, id: {id}");
        }
    }
}
