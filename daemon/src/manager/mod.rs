mod view;

use crate::{
    EmitEvent, Moxnotify, NotificationData,
    components::{
        Component, Data,
        notification::{self, Empty, Notification, NotificationId, NotificationState},
    },
    config::{Config, keymaps},
    history,
    rendering::texture_renderer::TextureArea,
    utils::{
        self, buffers,
        taffy::{GlobalLayout, NodeContext},
    },
};
use atomic_float::AtomicF32;
use calloop::LoopHandle;
use glyphon::{FontSystem, TextArea};
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
use taffy::style_helpers::auto;
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
    waiting: Vec<NotificationData>,
    config: Arc<Config>,
    loop_handle: LoopHandle<'static, Moxnotify>,
    sender: calloop::channel::Sender<crate::Event>,
    inhibited: bool,
    font_system: Rc<RefCell<FontSystem>>,
    pub notification_view: NotificationView,
    pub ui_state: UiState,
    pub history: history::History,
    pub tree: taffy::TaffyTree<NodeContext>,
    pub node_id: taffy::NodeId,
}

impl NotificationManager {
    pub fn new(
        config: Arc<Config>,
        loop_handle: LoopHandle<'static, Moxnotify>,
        sender: calloop::channel::Sender<crate::Event>,
        font_system: Rc<RefCell<FontSystem>>,
    ) -> Self {
        let ui_state = UiState::default();
        let mut tree = taffy::TaffyTree::new();

        Self {
            history: history::History::try_new(&config.general.history.path).unwrap(),
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
            node_id: tree
                .new_leaf(taffy::Style {
                    size: taffy::Size {
                        width: auto(),
                        height: auto(),
                    },
                    ..Default::default()
                })
                .unwrap(),
            tree,
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
        Vec<buffers::Instance>,
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
            .flat_map(|notification| notification.get_data(&self.tree, notification.urgency()))
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
                notification.get_render_bounds(&self.tree).x
                    + notification.get_render_bounds(&self.tree).width
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or_default();

        if let Some((instance, text_area)) =
            self.notification_view.prev_data(&self.tree, total_width)
        {
            instances.push(instance);
            text_areas.push(text_area);
        }

        if let Some((instance, text_area)) =
            self.notification_view.next_data(&self.tree, total_width)
        {
            instances.push(instance);
            text_areas.push(text_area);
        }

        (instances, text_areas, textures)
    }

    pub fn get_by_coordinates(&self, x: f64, y: f64) -> Option<&NotificationState> {
        self.iter_viewed().find(|notification| {
            let layout = self.tree.global_layout(notification.get_node_id()).unwrap();
            x >= layout.location.x as f64
                && x <= (layout.location.x + layout.content_box_width()) as f64
                && y >= layout.location.y as f64
                && y <= (layout.location.y + layout.content_box_height()) as f64
        })
    }

    pub fn click(&self, x: f64, y: f64) -> bool {
        self.get_by_coordinates(x, y)
            .map(|notification| {
                notification
                    .buttons()
                    .as_ref()
                    .is_some_and(|buttons| buttons.click(&self.tree, x, y))
            })
            .unwrap_or_default()
    }

    pub fn hover(&mut self, x: f64, y: f64) -> bool {
        self.notification_view.visible.clone().any(|index| {
            self.notifications
                .get_mut(index)
                .and_then(|notification| {
                    notification
                        .buttons_mut()
                        .map(|buttons| buttons.hover(&self.tree, x, y))
                })
                .unwrap_or_default()
        })
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

    pub fn add_many(&mut self, data: Vec<NotificationData>) {
        let nodes: Vec<_> = data
            .iter()
            .map(|_| self.tree.new_leaf(taffy::Style::DEFAULT).unwrap())
            .collect();

        let new_notifications: Vec<NotificationState> = data
            .into_par_iter()
            .zip(nodes.into_par_iter())
            .map(|(data, node)| {
                NotificationState::Empty(Notification::<Empty>::empty(
                    node,
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

    pub fn add(&mut self, data: NotificationData) {
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
                &mut self.tree,
                &mut self.font_system.borrow_mut(),
                data,
                Some(self.sender.clone()),
            );

            if self.notification_view.visible.contains(&i) {
                notification.start_timer(&self.loop_handle);
            }
        } else {
            let mut notification = Notification::<Empty>::empty(
                self.tree.new_leaf(taffy::Style::DEFAULT).unwrap(),
                Arc::clone(&self.config),
                data,
                self.ui_state.clone(),
            );

            match self.history.state() {
                history::HistoryState::Hidden => {
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
                history::HistoryState::Shown => self
                    .notifications
                    .push_front(NotificationState::Empty(notification)),
            }
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
                            self.tree.new_leaf(taffy::Style::DEFAULT).unwrap(),
                            Arc::clone(&self.config),
                            NotificationData::default(),
                            UiState::default(),
                        )),
                    )
                {
                    *notification_state = NotificationState::Ready(notification.promote(
                        &mut self.tree,
                        &mut self.font_system.borrow_mut(),
                        Some(self.sender.clone()),
                    ));
                }
            });
    }

    pub fn update_size(&mut self) {
        self.tree.clear();

        self.node_id = self
            .tree
            .new_leaf(taffy::Style {
                display: taffy::Display::Flex,
                flex_direction: taffy::FlexDirection::Column,
                size: taffy::Size {
                    width: auto(),
                    height: auto(),
                },
                ..Default::default()
            })
            .unwrap();

        self.notification_view
            .update_notification_count(&mut self.tree, self.notifications.len());

        if let Some(prev) = self.notification_view.prev.as_mut() {
            prev.update_layout(&mut self.tree);
            self.tree
                .add_child(self.node_id, prev.get_node_id())
                .unwrap();
        }

        self.notification_view.visible.clone().for_each(|i| {
            if let Some(notification) = self.notifications.get_mut(i) {
                notification.update_layout(&mut self.tree);
                self.tree
                    .add_child(self.node_id, notification.get_node_id())
                    .unwrap();
            }
        });

        if let Some(next) = self.notification_view.next.as_mut() {
            next.update_layout(&mut self.tree);
            self.tree
                .add_child(self.node_id, next.get_node_id())
                .unwrap();
        }

        self.tree
            .compute_layout_with_measure(
                self.node_id,
                taffy::Size::max_content(),
                |known_dimensions, available_space, _node_id, node_context, _style| {
                    utils::taffy::measure_function(
                        known_dimensions,
                        available_space,
                        node_context,
                        &mut self.font_system.borrow_mut(),
                    )
                },
            )
            .unwrap();

        if let Some(prev) = self.notification_view.prev.as_mut() {
            prev.apply_computed_layout(&mut self.tree);
        }
        self.notification_view.visible.clone().for_each(|i| {
            if let Some(notification) = self.notifications.get_mut(i) {
                notification.apply_computed_layout(&mut self.tree);
            }
        });
        if let Some(next) = self.notification_view.next.as_mut() {
            next.apply_computed_layout(&mut self.tree);
        }
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
                _ = self
                    .emit_sender
                    .send(EmitEvent::NotificationClosed { id: *id, reason });
            }
        }

        if ids.len() == self.notifications.notifications.len() {
            self.notifications.notifications.clear();
            self.notifications.update_size();
            return;
        }

        for id in ids {
            self.notifications.dismiss_by_id(id)
        }
    }

    pub fn dismiss_with_reason(&mut self, id: u32, reason: Option<Reason>) {
        match self.notifications.history.state() {
            history::HistoryState::Shown => {
                _ = self.notifications.history.delete(id);
                self.notifications.dismiss_by_id(id);
            }
            history::HistoryState::Hidden => {
                if self.notifications.selected_id() == Some(id) {
                    self.notifications
                        .ui_state
                        .mode
                        .store(keymaps::Mode::Normal, Ordering::Relaxed);
                }

                self.notifications.dismiss_by_id(id);
                if let Some(reason) = reason {
                    _ = self
                        .emit_sender
                        .send(EmitEvent::NotificationClosed { id, reason });
                }
            }
        }

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

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    use calloop::EventLoop;
    use glyphon::FontSystem;

    use super::NotificationManager;
    use crate::{config::Config, dbus::xdg::NotificationData};

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
        manager.add(data);

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

        manager.add(data.clone());

        manager.add(data);

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

        manager.add_many(notifications);
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
        manager.add(data);

        assert_eq!(manager.notifications().len(), 1);

        manager.dismiss_by_id(123);
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
        manager.add(data);

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
            manager.add(data);
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
        manager.add(data);

        assert_eq!(manager.notifications().len(), 1);

        manager.inhibit();

        let data = NotificationData {
            id: 1,
            ..Default::default()
        };
        manager.add(data);

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
            body: "body".into(),
            summary: "summary".into(),
            ..Default::default()
        };
        manager.add(data);

        let data = manager.data();
        // Instances
        // Body, summary, notification and dismiss button
        assert_eq!(data.0.len(), 4);
        // Text areas
        // Body and summary and dismiss button
        assert_eq!(data.1.len(), 3);
        // No icons
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
        manager.add(data);

        if let Some(notification) = manager.notifications.get_mut(0) {
            notification.update_layout(&mut manager.tree, 10.0, 20.0);
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

        assert!(
            manager
                .get_by_coordinates(left + width / 2.0, top + height / 2.0)
                .is_some()
        );

        // Left edge
        assert!(
            manager
                .get_by_coordinates(left - epsilon, top + height / 2.0)
                .is_none()
        );
        assert!(
            manager
                .get_by_coordinates(left + epsilon, top + height / 2.0)
                .is_some()
        );

        // Right edge
        assert!(
            manager
                .get_by_coordinates(right - epsilon, top + height / 2.0)
                .is_some()
        );
        assert!(
            manager
                .get_by_coordinates(right + epsilon, top + height / 2.0)
                .is_none()
        );

        // Top edge
        assert!(
            manager
                .get_by_coordinates(left + width / 2.0, top - epsilon)
                .is_none()
        );
        assert!(
            manager
                .get_by_coordinates(left + width / 2.0, top)
                .is_some()
        );

        // Bottom edge
        assert!(
            manager
                .get_by_coordinates(left + width / 2.0, bottom)
                .is_some()
        );
        assert!(
            manager
                .get_by_coordinates(left + width / 2.0, bottom + 30.00)
                .is_some()
        );

        // Top-left corner
        assert!(
            manager
                .get_by_coordinates(left - epsilon, top - epsilon)
                .is_none()
        );
        assert!(manager.get_by_coordinates(left + epsilon, top).is_some());

        // Top-right corner
        assert!(manager.get_by_coordinates(right - epsilon, top).is_some());
        assert!(
            manager
                .get_by_coordinates(right + epsilon, top - epsilon)
                .is_none()
        );

        // Bottom-left corner
        assert!(manager.get_by_coordinates(left + epsilon, bottom).is_some());
        assert!(
            manager
                .get_by_coordinates(left - epsilon, bottom + epsilon)
                .is_none()
        );

        // Bottom-right corner
        assert!(
            manager
                .get_by_coordinates(right - epsilon, bottom)
                .is_some()
        );
        assert!(
            manager
                .get_by_coordinates(right + epsilon, bottom + epsilon)
                .is_none()
        );

        let center_notification =
            manager.get_by_coordinates(left + width / 2.0, top + height / 2.0);
        assert_eq!(center_notification.unwrap().id(), 1);

        assert!(manager.get_by_coordinates(15.0, 25.0).is_some());
        assert!(manager.get_by_coordinates(9.9, 25.0).is_none());
        assert!(manager.get_by_coordinates(right, bottom).is_some());
        assert!(
            manager
                .get_by_coordinates(right + epsilon, bottom + epsilon)
                .is_none()
        );
    }
}
