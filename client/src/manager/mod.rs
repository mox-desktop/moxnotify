mod view;

use crate::{
    EmitEvent, Moxnotify,
    components::{
        Component, Data,
        notification::{self, Empty, Notification, NotificationId, NotificationState},
    },
    config::{Config, keymaps},
    moxnotify::{common::CloseReason, types::NewNotification},
};
use atomic_float::AtomicF32;
use calloop::LoopHandle;
use glyphon::{FontSystem, TextArea};
use moxui::{shape_renderer, texture_renderer::TextureArea};
use rayon::prelude::*;
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
    notifications: VecDeque<NotificationState>,
    waiting: Vec<NewNotification>,
    config: Arc<Config>,
    loop_handle: LoopHandle<'static, Moxnotify>,
    sender: calloop::channel::Sender<crate::Event>,
    inhibited: bool,
    font_system: Rc<RefCell<FontSystem>>,
    pub notification_view: NotificationView,
    pub ui_state: UiState,
}

impl NotificationManager {
    pub fn new(
        config: Arc<Config>,
        loop_handle: LoopHandle<'static, Moxnotify>,
        sender: calloop::channel::Sender<crate::Event>,
        font_system: Rc<RefCell<FontSystem>>,
    ) -> Self {
        let ui_state = UiState::default();

        Self {
            sender,
            inhibited: false,
            waiting: Vec::new(),
            notification_view: NotificationView::new(
                Arc::clone(&config),
                ui_state.clone(),
                Rc::clone(&font_system),
            ),
            font_system,
            loop_handle,
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

    pub fn notifications(&self) -> &VecDeque<NotificationState> {
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
            .filter_map(|notification| match notification {
                NotificationState::Empty(_) => None,
                NotificationState::Ready(notification) => Some(notification),
            })
            .flat_map(|notification| notification.get_data(notification.urgency()))
            .for_each(|data_item| match data_item {
                Data::Instance(instance) => instances.push(instance),
                Data::TextArea(text_area) => text_areas.push(text_area),
                Data::Texture(texture) => textures.push(texture),
            });

        let total_width = self
            .iter_viewed()
            .filter_map(|notification| match notification {
                NotificationState::Empty(_) => None,
                NotificationState::Ready(notification) => Some(notification),
            })
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

    pub fn get_by_coordinates(&self, x: f64, y: f64) -> Option<&NotificationState> {
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
        let height = self
            .notification_view
            .prev
            .as_ref()
            .map_or(0., |n| n.get_bounds().height);

        self.iter_viewed().fold(height, |acc, notification| {
            acc + notification.get_bounds().height
        }) + self
            .notification_view
            .next
            .as_ref()
            .map_or(0., |n| n.get_bounds().height)
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
    pub fn selected_notification_mut(&mut self) -> Option<&mut NotificationState> {
        let id = self.selected_id();
        self.notifications
            .iter_mut()
            .find(|notification| Some(notification.id()) == id)
    }

    /// Selects a notification by its ID, updating the selection state and visible range.
    ///
    /// If the selected notification is outside the current visible range, it handles
    /// the viewport to bring it into view. Also promotes unloaded notifications.
    pub fn select(&mut self, id: NotificationId) {
        let Some(new_index) = self.notifications.iter().position(|n| n.id() == id) else {
            return;
        };

        let current_selected = self
            .selected_id()
            .and_then(|current_id| self.notifications.iter().position(|n| n.id() == current_id));

        let current_view_start = self.notification_view.visible.start;
        let current_view_end = self.notification_view.visible.end;
        let max_visible = self.config.general.max_visible;

        if new_index < current_view_start || new_index >= current_view_end {
            match current_selected {
                // Moving up
                Some(old_index) if new_index > old_index => {
                    self.deselect();
                    let new_start = new_index.saturating_sub(max_visible.saturating_sub(1));
                    self.notification_view.visible =
                        new_start..new_start.saturating_add(max_visible);
                }
                // Moving down
                Some(old_index) if new_index < old_index => {
                    self.deselect();
                    self.notification_view.visible =
                        new_index..new_index.saturating_add(max_visible);
                }
                None => {
                    let new_start = new_index.saturating_sub(max_visible / 2);
                    self.notification_view.visible =
                        new_start..new_start.saturating_add(max_visible);
                }
                _ => {}
            }
        }

        self.promote_notifications();

        let Some(NotificationState::Ready(notification)) = self.notifications.get_mut(new_index)
        else {
            unreachable!();
        };

        notification.hover();
        log::info!("Selected notification id: {id}");

        self.ui_state.selected_id.store(id, Ordering::Relaxed);
        self.ui_state.selected.store(true, Ordering::Relaxed);

        let loop_handle = self.loop_handle.clone();
        self.iter_viewed_mut()
            .for_each(|notification| notification.stop_timer(&loop_handle));
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

        let loop_handle = self.loop_handle.clone();
        self.iter_viewed_mut().for_each(|notification| {
            notification.start_timer(&loop_handle);
        });
    }

    /// Select next notification
    pub fn next(&mut self) {
        let next_notification_index = {
            let id = self.ui_state.selected_id.load(Ordering::Relaxed);
            self.notifications
                .iter()
                .position(|n| n.id() == id)
                .map_or(0, |index| {
                    if index + 1 < self.notifications.len() {
                        index + 1
                    } else {
                        0
                    }
                })
        };

        if let Some(notification) = self.notifications.get(next_notification_index) {
            self.select(notification.id());
        }
    }

    /// Select previous notification
    pub fn prev(&mut self) {
        let notification_index = {
            let id = self.ui_state.selected_id.load(Ordering::Relaxed);
            self.notifications.iter().position(|n| n.id() == id).map_or(
                self.notifications.len().saturating_sub(1),
                |index| {
                    if index > 0 {
                        index - 1
                    } else {
                        self.notifications.len().saturating_sub(1)
                    }
                },
            )
        };

        if let Some(notification) = self.notifications.get(notification_index) {
            self.select(notification.id());
        }
    }

    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    pub fn add_many(&mut self, data: Vec<NewNotification>) {
        let new_notifications: Vec<NotificationState> = data
            .into_par_iter()
            .map(|data| {
                NotificationState::Empty(Notification::<Empty>::empty(
                    Arc::clone(&self.config),
                    data,
                    self.ui_state.clone(),
                ))
            })
            .collect();

        self.notifications.extend(new_notifications);
        self.promote_notifications();
        self.update_size();
    }

    pub fn add(&mut self, data: NewNotification) {
        if self.inhibited() {
            self.waiting.push(data);
            return;
        }

        if let Some((i, notification)) = self
            .notifications
            .iter_mut()
            .enumerate()
            .find(|(_, n)| n.id() == data.id)
        {
            notification.replace(
                &mut self.font_system.borrow_mut(),
                data,
                Some(self.sender.clone()),
            );

            if self.notification_view.visible.contains(&i) {
                notification.start_timer(&self.loop_handle);
            }
        } else {
            let mut notification =
                Notification::<Empty>::empty(Arc::clone(&self.config), data, self.ui_state.clone());

            if self
                .notification_view
                .visible
                .contains(&self.notifications.len())
            {
                notification.start_timer(&self.loop_handle);
            }

            self.notifications
                .push_back(NotificationState::Empty(notification));
        }

        self.promote_notifications();
        self.update_size();
    }

    pub fn dismiss_by_id(&mut self, id: NotificationId) {
        let Some(index) = self.notifications.iter().position(|n| n.id() == id) else {
            return;
        };

        if self.selected_id().is_some() {
            let next_notification = self.notifications.get(index + 1);

            match next_notification {
                Some(notification) if self.notification_view.visible.contains(&(index + 1)) => {
                    self.select(notification.id());
                }
                Some(notification) => {
                    self.select(notification.id());
                    self.notification_view.visible =
                        self.notification_view.visible.start.saturating_sub(1)
                            ..self.notification_view.visible.end.saturating_sub(1);
                    self.update_size();
                }
                None => {
                    self.prev();
                }
            }
        }

        self.notifications.remove(index);
        self.promote_notifications();

        if self.notifications.is_empty() {
            self.deselect();
        }
    }

    /// Returns an iterator over notifications in view
    pub fn iter_viewed(&self) -> impl Iterator<Item = &NotificationState> {
        self.notification_view
            .visible
            .clone()
            .filter_map(|idx| self.notifications.get(idx))
    }

    /// Returns an iterator over notifications in view that returns mutable references
    pub fn iter_viewed_mut(&mut self) -> impl Iterator<Item = &mut NotificationState> {
        self.notifications
            .iter_mut()
            .enumerate()
            .filter_map(|(i, notification)| {
                if self.notification_view.visible.contains(&i) {
                    Some(notification)
                } else {
                    None
                }
            })
    }

    /// Promotes notifications from Empty to Ready
    pub fn promote_notifications(&mut self) {
        self.notification_view
            .visible
            .clone()
            .for_each(|notification_idx| {
                if let Some(notification_state) = self.notifications.get_mut(notification_idx)
                    && matches!(notification_state, NotificationState::Empty(_))
                    && let NotificationState::Empty(notification) = std::mem::replace(
                        notification_state,
                        NotificationState::Empty(Notification::<Empty>::empty(
                            Arc::clone(&self.config),
                            NewNotification::default(),
                            UiState::default(),
                        )),
                    )
                {
                    *notification_state = NotificationState::Ready(notification.promote(
                        &mut self.font_system.borrow_mut(),
                        Some(self.sender.clone()),
                    ));
                }
            });
    }

    pub fn update_size(&mut self) {
        let x_offset = self
            .iter_viewed()
            .map(|notification| notification.data().hints.as_ref().unwrap().x)
            .min()
            .unwrap_or_default()
            .abs() as f32;

        if let Some(prev) = self.notification_view.prev.as_mut() {
            prev.set_position(0., 0.);
        }

        let mut start = self
            .notification_view
            .prev
            .as_ref()
            .map(|n| n.get_bounds().y + n.get_bounds().height)
            .unwrap_or_default();

        self.iter_viewed_mut().for_each(|notification| {
            notification.set_position(x_offset, start);
            start += notification.get_bounds().height;
        });

        if let Some(next) = self.notification_view.next.as_mut() {
            next.set_position(0., start);
        }

        self.notification_view
            .update_notification_count(self.notifications.len());
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
            .map(notification::NotificationState::id)
            .collect();

        if let Some(reason) = reason {
            for id in &ids {
                let uuid = self.notifications.iter_viewed().find_map(|notification| {
                    if notification.id() == *id {
                        Some(notification.uuid())
                    } else {
                        None
                    }
                });

                _ = self.emit_sender.send(EmitEvent::NotificationClosed {
                    id: *id,
                    reason,
                    uuid: uuid.unwrap(),
                });
            }
        }

        if ids.len() == self.notifications.notifications.len() {
            self.notifications.notifications.clear();
            self.notifications.update_size();
            return;
        }

        ids.iter()
            .for_each(|id| self.notifications.dismiss_by_id(*id));
    }

    pub fn dismiss_with_reason(&mut self, id: u32, reason: CloseReason) {
        if self.notifications.selected_id() == Some(id) {
            self.notifications
                .ui_state
                .mode
                .store(keymaps::Mode::Normal, Ordering::Relaxed);
        }

        let uuid = self.notifications.iter_viewed().find_map(|notification| {
            if notification.id() == id {
                Some(notification.uuid())
            } else {
                None
            }
        });

        self.notifications.dismiss_by_id(id);
        _ = self.emit_sender.send(EmitEvent::NotificationClosed {
            id,
            reason,
            uuid: uuid.unwrap(),
        });

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
    }
}
