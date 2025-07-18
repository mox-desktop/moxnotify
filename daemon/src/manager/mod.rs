mod view;

use crate::{
    components::{
        button::ButtonType,
        notification::{Notification, NotificationId},
        text::Text,
        Component, Data,
    },
    config::{keymaps, Config, Queue},
    rendering::texture_renderer::TextureArea,
    utils::buffers,
    EmitEvent, History, Moxnotify, NotificationData,
};
use atomic_float::AtomicF32;
use calloop::LoopHandle;
use glyphon::{FontSystem, TextArea};
use rayon::prelude::*;
use rusqlite::params;
use std::{
    cell::RefCell,
    fmt,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
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
    notifications: Vec<Notification>,
    waiting: u32,
    config: Arc<Config>,
    loop_handle: LoopHandle<'static, Moxnotify>,
    pub font_system: Rc<RefCell<FontSystem>>,
    pub notification_view: NotificationView,
    sender: calloop::channel::Sender<crate::Event>,
    inhibited: bool,
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
            waiting: 0,
            notification_view: NotificationView::new(
                Arc::clone(&config),
                ui_state.clone(),
                Rc::clone(&font_system),
            ),
            font_system,
            loop_handle,
            notifications: Vec::new(),
            config,
            ui_state,
        }
    }

    pub fn inhibit(&mut self) {
        self.inhibited = true;
    }

    pub fn uninhibit(&mut self) {
        self.waiting = 0;
        self.inhibited = false;
    }

    pub fn inhibited(&mut self) -> bool {
        self.inhibited
    }

    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
    }

    pub fn data(
        &self,
    ) -> (
        Vec<buffers::Instance>,
        Vec<TextArea<'_>>,
        Vec<TextureArea<'_>>,
    ) {
        let mut instances = Vec::new();
        let mut text_areas = Vec::new();
        let mut textures = Vec::new();

        let all_data: Vec<Data> = self
            .notifications
            .iter()
            .enumerate()
            .filter(|(i, _)| self.notification_view.visible.contains(i))
            .flat_map(|(_, notification)| notification.get_data(notification.urgency()))
            .collect();

        for data_item in all_data {
            match data_item {
                Data::Instance(instance) => instances.push(instance),
                Data::TextArea(text_area) => text_areas.push(text_area),
                Data::Texture(texture) => textures.push(texture),
            }
        }

        let total_width = self
            .notifications
            .iter()
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
        self.notification_view
            .visible
            .clone()
            .filter_map(|index| {
                if let Some(notification) = self.notifications.get(index) {
                    let extents = notification.get_render_bounds();
                    let x_within_bounds =
                        x >= extents.x as f64 && x <= (extents.x + extents.width) as f64;
                    let y_within_bounds =
                        y >= extents.y as f64 && y <= (extents.y + extents.height) as f64;

                    if x_within_bounds && y_within_bounds {
                        return Some(notification);
                    }
                }

                None
            })
            .next()
    }

    pub fn click(&mut self, x: f64, y: f64) -> bool {
        self.notification_view.visible.clone().any(|index| {
            self.notifications
                .get_mut(index)
                .map(|notification| notification.buttons.click(x, y))
                .unwrap_or_default()
        })
    }

    pub fn hover(&mut self, x: f64, y: f64) -> bool {
        self.notification_view.visible.clone().any(|index| {
            self.notifications
                .get_mut(index)
                .map(|notification| notification.buttons.hover(x, y))
                .unwrap_or_default()
        })
    }

    pub fn height(&self) -> f32 {
        let height = self
            .notification_view
            .prev
            .as_ref()
            .map_or(0., |n| n.get_bounds().height);
        self.notification_view
            .visible
            .clone()
            .fold(height, |acc, i| {
                if let Some(notification) = self.notifications.get(i) {
                    let extents = notification.get_bounds();
                    return acc + extents.height;
                };

                acc
            })
            + self
                .notification_view
                .next
                .as_ref()
                .map_or(0., |n| n.get_bounds().height)
    }

    pub fn width(&self) -> f32 {
        let (min_x, max_x) =
            self.notifications
                .iter()
                .fold((f32::MAX, f32::MIN), |(min_x, max_x), notification| {
                    let extents = notification.get_bounds();
                    let left = extents.x + notification.data.hints.x as f32;
                    let right = extents.x + extents.width + notification.data.hints.x as f32;
                    (min_x.min(left), max_x.max(right))
                });

        if min_x == f32::MAX || max_x == f32::MIN {
            0.0
        } else {
            max_x - min_x
        }
    }

    pub fn selected_id(&self) -> Option<NotificationId> {
        match self.ui_state.selected.load(Ordering::Relaxed) {
            true => Some(self.ui_state.selected_id.load(Ordering::Relaxed)),
            false => None,
        }
    }

    pub fn selected_notification_mut(&mut self) -> Option<&mut Notification> {
        let id = self.selected_id();
        self.notifications
            .iter_mut()
            .find(|notification| Some(notification.id()) == id)
    }

    pub fn select(&mut self, id: NotificationId) {
        self.deselect();

        if let Some(notification) = self.notifications.iter_mut().find(|n| n.id() == id) {
            notification.hover();
            log::info!("Selected notification id: {id}");

            self.ui_state.selected_id.store(id, Ordering::Relaxed);
            self.ui_state.selected.store(true, Ordering::Relaxed);

            notification.stop_timer(&self.loop_handle);

            let dismiss_button = notification
                .buttons
                .buttons()
                .iter()
                .find(|button| button.button_type() == ButtonType::Dismiss)
                .map(|button| button.get_render_bounds().width)
                .unwrap_or(0.0);

            notification.body.set_size(
                &mut self.font_system.borrow_mut(),
                Some(
                    notification.get_style().width
                        - notification.icons.get_bounds().width
                        - dismiss_button,
                ),
                None,
            );

            notification.summary.set_size(
                &mut self.font_system.borrow_mut(),
                Some(
                    notification.get_style().width
                        - notification.icons.get_bounds().width
                        - dismiss_button,
                ),
                None,
            );
        }
    }

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
            self.notification_view.next(
                self.height(),
                next_notification_index,
                self.notifications.len(),
            );
        }

        self.notification_view.visible.clone().fold(
            self.notification_view
                .prev
                .as_ref()
                .map(|p| p.get_bounds().height)
                .unwrap_or(0.),
            |acc, i| {
                if let Some(notification) = self.notifications.get_mut(i) {
                    notification.set_position(notification.x, acc);
                    acc + notification.get_bounds().height
                } else {
                    acc
                }
            },
        );

        self.notification_view
            .update_notification_count(self.height(), self.notifications.len());
    }

    pub fn prev(&mut self) {
        if !self.ui_state.selected.load(Ordering::Relaxed) {
            return;
        }

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
            self.notification_view.prev(
                self.height(),
                notification_index,
                self.notifications.len(),
            );
        }

        self.notification_view.visible.clone().fold(
            self.notification_view
                .prev
                .as_ref()
                .map(|p| p.get_bounds().height)
                .unwrap_or(0.),
            |acc, i| {
                if let Some(notification) = self.notifications.get_mut(i) {
                    notification.set_position(notification.x, acc);
                    acc + notification.get_bounds().height
                } else {
                    acc
                }
            },
        );

        self.notification_view
            .update_notification_count(self.height(), self.notifications.len());
    }

    pub fn deselect(&mut self) {
        if !self.ui_state.selected.load(Ordering::Relaxed) {
            return;
        }

        self.ui_state.selected.store(false, Ordering::Relaxed);

        let old_id = self.ui_state.selected_id.load(Ordering::Relaxed);
        if let Some(index) = self.notifications.iter().position(|n| n.id() == old_id) {
            if let Some(notification) = self.notifications.get_mut(index) {
                notification.unhover();
                match self.config.general.queue {
                    Queue::FIFO if index == 0 => notification.start_timer(&self.loop_handle),
                    Queue::Unordered => notification.start_timer(&self.loop_handle),
                    _ => {}
                }
            }
        }
    }

    pub fn waiting(&self) -> u32 {
        self.waiting
    }

    pub fn add_many(&mut self, data: Vec<NotificationData>) -> anyhow::Result<()> {
        let a = std::time::Instant::now();
        let new_notifications: Vec<Notification> = data
            .into_par_iter()
            .map_init(
                FontSystem::new, // Initialize font system once per thread
                |font_system, data| {
                    Notification::new(
                        Arc::clone(&self.config),
                        font_system,
                        data,
                        self.ui_state.clone(),
                        None,
                    )
                },
            )
            .collect();
        println!("{:?}", a.elapsed());

        self.notifications.extend(new_notifications);

        let mut y = self
            .notifications
            .last()
            .map(|notification| notification.y)
            .unwrap_or_default();
        self.notifications.iter_mut().for_each(|notification| {
            notification.set_position(0.0, y);

            let height = notification.get_bounds().height;
            y += height;
        });

        if self.notification_view.visible.end < self.notifications.len() {
            self.notification_view
                .update_notification_count(self.height(), self.notifications.len());
        }

        let x_offset = self
            .notifications
            .iter()
            .map(|n| n.data.hints.x)
            .min()
            .unwrap_or_default()
            .abs() as f32;

        self.notifications
            .iter_mut()
            .for_each(|n| n.set_position(x_offset, n.y));
        Ok(())
    }

    pub fn add(&mut self, data: NotificationData) -> anyhow::Result<()> {
        if self.inhibited {
            self.waiting += 1;
            return Ok(());
        }

        let id = data.id;
        let (y, existing_index) =
            if let Some(index) = self.notifications.iter().position(|n| n.id() == id) {
                let y = self.notifications[index].get_bounds().y;
                (y, Some(index))
            } else {
                (self.height(), None)
            };

        let mut notification = Notification::new(
            Arc::clone(&self.config),
            &mut self.font_system.borrow_mut(),
            data,
            self.ui_state.clone(),
            Some(self.sender.clone()),
        );
        notification.set_position(0.0, y);

        match self.config.general.queue {
            Queue::FIFO if self.notifications.is_empty() => {
                notification.start_timer(&self.loop_handle)
            }

            Queue::Unordered => notification.start_timer(&self.loop_handle),
            _ => {}
        }

        match existing_index {
            Some(index) => {
                if let Some(notification) = self.notifications.get(index) {
                    notification.stop_timer(&self.loop_handle);
                }

                let replaced_height_differs = self.notifications[index].get_bounds().height
                    != notification.get_bounds().height;

                self.notifications[index] = notification;

                if replaced_height_differs {
                    self.notification_view.visible.clone().fold(
                        self.notification_view
                            .prev
                            .as_ref()
                            .map(|p| p.get_bounds().height)
                            .unwrap_or(0.),
                        |acc, i| {
                            if let Some(notification) = self.notifications.get_mut(i) {
                                notification.set_position(notification.x, acc);
                                acc + notification.get_bounds().height
                            } else {
                                acc
                            }
                        },
                    );
                }
            }
            None => self.notifications.push(notification),
        }

        // Maintain selection if replaced
        if let Some(id) = self.selected_id() {
            self.select(id);
        }

        if self.notification_view.visible.end < self.notifications.len() {
            self.notification_view
                .update_notification_count(self.height(), self.notifications.len());
        }

        let x_offset = self
            .notifications
            .iter()
            .map(|n| n.data.hints.x)
            .min()
            .unwrap_or_default()
            .abs() as f32;

        self.notifications
            .iter_mut()
            .for_each(|n| n.set_position(x_offset, n.y));

        Ok(())
    }

    pub fn dismiss(&mut self, id: NotificationId) {
        if let Some(i) = self.notifications.iter().position(|n| n.id() == id) {
            if let Some(notification) = self.notifications.get(i) {
                notification.stop_timer(&self.loop_handle);

                if let Some(next_notification) = self.notifications.get(i + 1) {
                    if self.selected_id() == Some(notification.id()) {
                        self.select(next_notification.id());
                    }
                    self.notifications.remove(i);
                } else {
                    self.notifications.remove(i);
                    self.prev();
                }
            }
        }

        self.notification_view
            .update_notification_count(self.height(), self.notifications.len());

        if let Queue::FIFO = self.config.general.queue {
            if let Some(notification) = self.notifications.first_mut().filter(|n| !n.hovered()) {
                notification.start_timer(&self.loop_handle);
            }
        }

        self.notification_view.visible.clone().fold(
            self.notification_view
                .prev
                .as_ref()
                .map(|p| p.get_bounds().height)
                .unwrap_or(0.),
            |acc, i| {
                if let Some(notification) = self.notifications.get_mut(i) {
                    notification.set_position(notification.x, acc);
                    acc + notification.get_bounds().height
                } else {
                    acc
                }
            },
        );
    }
}

#[derive(Clone, Copy)]
pub enum Reason {
    Expired = 1,
    DismissedByUser = 2,
    CloseNotificationCall = 3,
    Unkown = 4,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Reason::Expired => "Expired",
            Reason::DismissedByUser => "DismissedByUser",
            Reason::CloseNotificationCall => "CloseNotificationCall",
            Reason::Unkown => "Unknown",
        };
        write!(f, "{s}")
    }
}

impl Moxnotify {
    pub fn dismiss_range<T>(&mut self, range: T, reason: Option<Reason>)
    where
        T: std::slice::SliceIndex<[Notification], Output = [Notification]>,
    {
        let ids: Vec<_> = self.notifications.notifications()[range]
            .iter()
            .map(|notification| notification.id())
            .collect();

        if let Some(reason) = reason {
            ids.iter().for_each(|id| {
                _ = self
                    .emit_sender
                    .send(EmitEvent::NotificationClosed { id: *id, reason });
            });
        }

        if ids.len() == self.notifications.notifications.len() {
            self.notifications.notifications.clear();
            self.notifications
                .notification_view
                .update_notification_count(0., 0);
            return;
        }

        ids.iter().for_each(|id| self.notifications.dismiss(*id));
    }

    pub fn dismiss_by_id(&mut self, id: u32, reason: Option<Reason>) {
        match self.history {
            History::Shown => {
                _ = self
                    .db
                    .execute("DELETE FROM notifications WHERE rowid = ?1", params![id]);
                self.notifications.dismiss(id);
            }
            History::Hidden => {
                if let Some(index) = self
                    .notifications
                    .notifications
                    .iter()
                    .position(|n| n.id() == id)
                {
                    if self.notifications.selected_id() == Some(id) {
                        self.notifications
                            .ui_state
                            .mode
                            .store(keymaps::Mode::Normal, Ordering::Relaxed);
                    }

                    self.notifications.dismiss(id);
                    if let Some(reason) = reason {
                        _ = self
                            .emit_sender
                            .send(EmitEvent::NotificationClosed { id, reason });
                    }
                    if self.notifications.selected_id() == Some(id) {
                        let new_index = if index >= self.notifications.notifications.len() {
                            self.notifications.notifications.len().saturating_sub(1)
                        } else {
                            index
                        };

                        if let Some(notification) = self.notifications.notifications.get(new_index)
                        {
                            self.notifications.select(notification.id());
                        }
                    }
                }
            }
        }

        self.update_surface_size();
        if let Some(surface) = self.surface.as_mut() {
            if let Err(e) = surface.render(
                &self.wgpu_state.device,
                &self.wgpu_state.queue,
                &self.notifications,
            ) {
                log::error!("Render error: {e}");
            }
        }

        if self.notifications.notifications().is_empty() {
            self.seat.keyboard.repeat.key = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    use calloop::EventLoop;
    use glyphon::FontSystem;

    use super::NotificationManager;
    use crate::{components::Component, config::Config, dbus::xdg::NotificationData};

    #[test]
    fn test_add() {
        let config = Arc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager = NotificationManager::new(
            Arc::clone(&config),
            event_loop.handle(),
            calloop::channel::channel().0,
            font_system,
        );

        let data = NotificationData::default();
        manager.add(data).unwrap();

        assert_eq!(manager.notifications().len(), 1);
    }

    #[test]
    fn test_add_with_duplicate_id() {
        let config = Arc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager = NotificationManager::new(
            Arc::clone(&config),
            event_loop.handle(),
            calloop::channel::channel().0,
            font_system,
        );

        let data = NotificationData {
            id: 42,
            ..Default::default()
        };

        manager.add(data.clone()).unwrap();

        manager.add(data).unwrap();

        assert_eq!(manager.notifications().len(), 1);
    }

    #[test]
    fn test_add_many() {
        let config = Arc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager = NotificationManager::new(
            Arc::clone(&config),
            event_loop.handle(),
            calloop::channel::channel().0,
            font_system,
        );

        let mut notifications = Vec::new();
        for i in 1..=5 {
            let data = NotificationData {
                id: i,
                ..Default::default()
            };
            notifications.push(data);
        }

        manager.add_many(notifications).unwrap();
        assert_eq!(manager.notifications().len(), 5);
    }

    #[test]
    fn test_dismiss() {
        let config = Arc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager = NotificationManager::new(
            Arc::clone(&config),
            event_loop.handle(),
            calloop::channel::channel().0,
            font_system,
        );

        let data = NotificationData {
            id: 123,
            ..Default::default()
        };
        manager.add(data).unwrap();

        assert_eq!(manager.notifications().len(), 1);

        manager.dismiss(123);
        assert_eq!(manager.notifications().len(), 0);
    }

    #[test]
    fn test_select_and_deselect() {
        let config = Arc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager = NotificationManager::new(
            Arc::clone(&config),
            event_loop.handle(),
            calloop::channel::channel().0,
            font_system,
        );

        let data = NotificationData {
            id: 1,
            ..Default::default()
        };
        manager.add(data).unwrap();

        assert_eq!(manager.selected_id(), None);

        manager.select(1);
        assert_eq!(manager.selected_id(), Some(1));

        let notification = manager.selected_notification_mut().unwrap();
        assert!(notification.hovered());

        manager.deselect();
        assert_eq!(manager.selected_id(), None);
    }

    #[test]
    fn test_next_and_prev() {
        let config = Arc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager = NotificationManager::new(
            Arc::clone(&config),
            event_loop.handle(),
            calloop::channel::channel().0,
            font_system,
        );

        for i in 1..=10 {
            let data = NotificationData {
                id: i,
                ..Default::default()
            };
            manager.add(data).unwrap();
        }

        manager.next();
        assert_eq!(manager.selected_id(), Some(1));

        manager.next();
        assert_eq!(manager.selected_id(), Some(2));

        manager.next();
        assert_eq!(manager.selected_id(), Some(3));

        manager.next();
        assert_eq!(manager.selected_id(), Some(4));

        manager.next();
        assert_eq!(manager.selected_id(), Some(5));

        manager.next();
        assert_eq!(manager.selected_id(), Some(6));

        manager.prev();
        assert_eq!(manager.selected_id(), Some(5));

        manager.prev();
        assert_eq!(manager.selected_id(), Some(4));

        manager.prev();
        assert_eq!(manager.selected_id(), Some(3));

        manager.prev();
        assert_eq!(manager.selected_id(), Some(2));

        manager.prev();
        assert_eq!(manager.selected_id(), Some(1));
    }

    #[test]
    fn test_inhibit() {
        let config = Arc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager = NotificationManager::new(
            Arc::clone(&config),
            event_loop.handle(),
            calloop::channel::channel().0,
            font_system,
        );

        let data = NotificationData {
            id: 0,
            ..Default::default()
        };
        manager.add(data).unwrap();

        assert_eq!(manager.notifications().len(), 1);

        manager.inhibit();

        let data = NotificationData {
            id: 1,
            ..Default::default()
        };
        manager.add(data).unwrap();

        assert!(manager.inhibited());
        assert_eq!(manager.notifications().len(), 1);
        assert_eq!(manager.waiting(), 1);

        manager.uninhibit();

        assert!(!manager.inhibited());
        assert_eq!(manager.notifications().len(), 1);
        assert_eq!(manager.waiting(), 0);
    }

    #[test]
    fn test_data() {
        let config = Arc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager = NotificationManager::new(
            Arc::clone(&config),
            event_loop.handle(),
            calloop::channel::channel().0,
            font_system,
        );

        let data = NotificationData {
            id: 123,
            ..Default::default()
        };
        manager.add(data).unwrap();

        let data = manager.data();
        // Body, summary, notification and dismiss button
        assert_eq!(data.0.len(), 4);
        // Body and summary
        assert_eq!(data.1.len(), 2);
        assert_eq!(data.2.len(), 0);
    }

    #[test]
    fn test_get_by_coordinates() {
        let config = Arc::new(Config::default());
        let style = &config.styles.default;
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager = NotificationManager::new(
            Arc::clone(&config),
            event_loop.handle(),
            calloop::channel::channel().0,
            font_system,
        );

        let data = NotificationData {
            id: 1,
            ..Default::default()
        };
        manager.add(data).unwrap();

        if let Some(notification) = manager.notifications.get_mut(0) {
            notification.set_position(10.0, 20.0);
        }

        let x = 10.0 + style.margin.left.resolve(0.) as f64;
        let y = 20.0 + style.margin.top.resolve(0.) as f64;
        let width = (style.width
            + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right) as f64;
        let height = (style.height
            + style.border.size.top
            + style.border.size.bottom
            + style.padding.top
            + style.padding.bottom) as f64;

        let (left, right, top, bottom) = (x, x + width, y, y + height);
        let epsilon = 0.1;

        assert!(manager
            .get_by_coordinates(left + width / 2.0, top + height / 2.0)
            .is_some());

        // Left edge
        assert!(manager
            .get_by_coordinates(left - epsilon, top + height / 2.0)
            .is_none());
        assert!(manager
            .get_by_coordinates(left + epsilon, top + height / 2.0)
            .is_some());

        // Right edge
        assert!(manager
            .get_by_coordinates(right - epsilon, top + height / 2.0)
            .is_some());
        assert!(manager
            .get_by_coordinates(right + epsilon, top + height / 2.0)
            .is_none());

        // Top edge
        assert!(manager
            .get_by_coordinates(left + width / 2.0, top - epsilon)
            .is_none());
        assert!(manager
            .get_by_coordinates(left + width / 2.0, top)
            .is_some());

        // Bottom edge
        assert!(manager
            .get_by_coordinates(left + width / 2.0, bottom)
            .is_some());
        assert!(manager
            .get_by_coordinates(left + width / 2.0, bottom + 30.00)
            .is_some());

        // Top-left corner
        assert!(manager
            .get_by_coordinates(left - epsilon, top - epsilon)
            .is_none());
        assert!(manager.get_by_coordinates(left + epsilon, top).is_some());

        // Top-right corner
        assert!(manager.get_by_coordinates(right - epsilon, top).is_some());
        assert!(manager
            .get_by_coordinates(right + epsilon, top - epsilon)
            .is_none());

        // Bottom-left corner
        assert!(manager.get_by_coordinates(left + epsilon, bottom).is_some());
        assert!(manager
            .get_by_coordinates(left - epsilon, bottom + epsilon)
            .is_none());

        // Bottom-right corner
        assert!(manager
            .get_by_coordinates(right - epsilon, bottom)
            .is_some());
        assert!(manager
            .get_by_coordinates(right + epsilon, bottom + epsilon)
            .is_none());

        let center_notification =
            manager.get_by_coordinates(left + width / 2.0, top + height / 2.0);
        assert_eq!(center_notification.unwrap().id(), 1);

        assert!(manager.get_by_coordinates(15.0, 25.0).is_some());
        assert!(manager.get_by_coordinates(9.9, 25.0).is_none());
        assert!(manager.get_by_coordinates(right, bottom).is_some());
        assert!(manager
            .get_by_coordinates(right + epsilon, bottom + epsilon)
            .is_none());
    }
}
