use super::button::{ButtonManager, ButtonType, Finished};
use super::icons::Icons;
use super::progress::Progress;
use super::text::Text;
use super::text::body::Body;
use super::text::summary::Summary;
use super::{Bounds, UiState};
use crate::components;
use crate::manager::Reason;
use crate::rendering::texture_renderer;
use crate::{
    Config, Moxnotify, NotificationData, Urgency,
    components::{Component, Data},
    config::StyleState,
    utils::{buffers, taffy::GlobalLayout},
};
use calloop::{
    LoopHandle, RegistrationToken,
    timer::{TimeoutAction, Timer},
};
use glyphon::FontSystem;
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use taffy::style_helpers::{auto, fr, length, max_content};

pub enum NotificationState {
    Empty(Notification<Empty>),
    Ready(Notification<Ready>),
}

impl NotificationState {
    #[must_use]
    pub fn id(&self) -> NotificationId {
        match self {
            Self::Empty(n) => n.id(),
            Self::Ready(n) => n.id(),
        }
    }

    #[must_use]
    pub fn data(&self) -> &NotificationData {
        match self {
            Self::Empty(n) => &n.data,
            Self::Ready(n) => &n.data,
        }
    }

    pub fn start_timer(&mut self, loop_handle: &LoopHandle<'static, Moxnotify>) {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.start_timer(loop_handle),
        }
    }

    pub fn stop_timer(&mut self, loop_handle: &LoopHandle<'static, Moxnotify>) {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.stop_timer(loop_handle),
        }
    }

    pub fn update_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.update_layout(tree),
        }
    }

    pub fn apply_computed_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.apply_computed_layout(tree),
        }
    }

    pub fn get_node_id(&self) -> taffy::NodeId {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.get_node_id(),
        }
    }

    #[must_use]
    pub fn hovered(&self) -> bool {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.hovered(),
        }
    }

    #[must_use]
    pub fn get_bounds(&self, tree: &taffy::TaffyTree<()>) -> Bounds {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.get_bounds(tree),
        }
    }

    #[must_use]
    pub fn get_render_bounds(&self, tree: &taffy::TaffyTree<()>) -> Bounds {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.get_render_bounds(tree),
        }
    }

    pub fn unhover(&mut self) {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.unhover(),
        }
    }

    pub fn replace(
        &mut self,
        tree: &mut taffy::TaffyTree<()>,
        font_system: &mut FontSystem,
        data: NotificationData,
        sender: Option<calloop::channel::Sender<crate::Event>>,
    ) {
        match self {
            Self::Empty(n) => unreachable!(),
            Self::Ready(n) => n.replace(tree, font_system, data, sender),
        }
    }

    #[must_use]
    pub fn buttons(&self) -> Option<&ButtonManager<Finished>> {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.buttons.as_ref(),
        }
    }

    pub fn buttons_mut(&mut self) -> Option<&mut ButtonManager<Finished>> {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.buttons.as_mut(),
        }
    }
}

pub type NotificationId = u32;

pub struct Notification<State> {
    pub y: f32,
    pub x: f32,
    hovered: bool,
    pub icons: Option<Icons>,
    progress: Option<Progress>,
    pub registration_token: Option<RegistrationToken>,
    pub buttons: Option<ButtonManager<Finished>>,
    pub data: NotificationData,
    pub summary: Option<Summary>,
    pub body: Option<Body>,
    context: components::Context,
    node: taffy::NodeId,
    _state: std::marker::PhantomData<State>,
}

impl<State> PartialEq for Notification<State> {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Component for Notification<Ready> {
    type Style = StyleState;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        self.get_notification_style()
    }

    fn get_instances(
        &self,
        tree: &taffy::TaffyTree<()>,
        urgency: Urgency,
    ) -> Vec<buffers::Instance> {
        let extents = self.get_render_bounds(tree);
        let style = self.get_style();

        vec![buffers::Instance {
            rect_pos: [extents.x, extents.y],
            rect_size: [
                extents.width - style.border.size.left - style.border.size.right,
                extents.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.color(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.color(urgency),
            scale: self.get_ui_state().scale.load(Ordering::Relaxed),
            depth: 0.9,
        }]
    }

    fn get_text_areas(
        &self,
        _: &taffy::TaffyTree<()>,
        _: Urgency,
    ) -> Vec<glyphon::TextArea<'_>> {
        Vec::new()
    }

    fn get_textures(&self, _: &taffy::TaffyTree<()>) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        let style = self.get_style();
        self.node = tree
            .new_leaf(taffy::Style {
                size: taffy::Size {
                    width: if style.width.is_auto() {
                        auto()
                    } else {
                        length(style.width.resolve(0.))
                    },
                    height: if style.height.is_auto() {
                        auto()
                    } else {
                        length(style.height.resolve(0.))
                    },
                },
                padding: taffy::Rect {
                    left: length(style.padding.left.resolve(0.)),
                    right: length(style.padding.right.resolve(0.)),
                    top: length(style.padding.top.resolve(0.)),
                    bottom: length(style.padding.bottom.resolve(0.)),
                },
                margin: taffy::Rect {
                    left: if style.margin.left.is_auto() {
                        auto()
                    } else {
                        length(style.margin.left.resolve(0.))
                    },
                    right: if style.margin.right.is_auto() {
                        auto()
                    } else {
                        length(style.margin.right.resolve(0.))
                    },
                    top: if style.margin.top.is_auto() {
                        auto()
                    } else {
                        length(style.margin.top.resolve(0.))
                    },
                    bottom: if style.margin.bottom.is_auto() {
                        auto()
                    } else {
                        length(style.margin.bottom.resolve(0.))
                    },
                },
                border: taffy::Rect {
                    left: length(style.border.size.left.resolve(0.)),
                    right: length(style.border.size.left.resolve(0.)),
                    top: length(style.border.size.left.resolve(0.)),
                    bottom: length(style.border.size.left.resolve(0.)),
                },
                display: taffy::Display::Grid,
                grid_auto_rows: vec![max_content()],
                grid_template_rows: vec![auto(), auto(), auto(), auto()],
                grid_template_columns: vec![auto(), fr(1.), auto()],
                ..Default::default()
            })
            .unwrap();

        let container_node = self.get_node_id();

        if let Some(summary) = self.summary.as_mut() {
            summary.update_layout(tree);
            tree.add_child(container_node, summary.get_node_id())
                .unwrap();
        }

        if let Some(body) = self.body.as_mut() {
            body.update_layout(tree);
            tree.add_child(container_node, body.get_node_id()).unwrap();
        }

        if let Some(icons) = self.icons.as_mut() {
            icons.update_layout(tree);
            tree.add_child(container_node, icons.get_node_id()).unwrap();
        }

        if let Some(progress) = self.progress.as_mut() {
            progress.update_layout(tree);
            tree.add_child(container_node, progress.get_node_id())
                .unwrap();
        }

        if let Some(buttons) = self.buttons.as_mut() {
            let action_container = tree
                .new_leaf(taffy::Style {
                    display: taffy::Display::Flex,
                    flex_direction: taffy::FlexDirection::Row,
                    justify_content: Some(taffy::JustifyContent::SpaceEvenly),
                    size: taffy::Size {
                        width: auto(),
                        height: auto(),
                    },
                    ..Default::default()
                })
                .ok();

            buttons
                .buttons_mut()
                .iter_mut()
                .for_each(|button| match button.button_type() {
                    ButtonType::Action => {
                        if let Some(action_container) = action_container {
                            button.update_layout(tree);
                            tree.add_child(action_container, button.get_node_id())
                                .unwrap();
                        }
                    }
                    ButtonType::Dismiss | ButtonType::Anchor => {
                        button.update_layout(tree);
                        tree.add_child(container_node, button.get_node_id())
                            .unwrap();
                    }
                });

            if let Some(action_container) = action_container {
                tree.add_child(container_node, action_container).unwrap();
            }

            buttons.action_container = action_container;
        }
    }

    fn apply_computed_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        if let Some(icons) = self.icons.as_mut() {
            icons.apply_computed_layout(tree);
        }

        if let Some(progress) = self.progress.as_mut() {
            progress.apply_computed_layout(tree);
        }

        if let Some(summary) = self.summary.as_mut() {
            summary.apply_computed_layout(tree);
        }

        if let Some(body) = self.body.as_mut() {
            body.apply_computed_layout(tree);
        }

        self.buttons.as_mut().map(|buttons| {
            buttons.buttons_mut().iter_mut().for_each(|button| {
                button.apply_computed_layout(tree);
            })
        });

        let layout = tree.global_layout(self.get_node_id()).unwrap();
        self.x = layout.location.x;
        self.y = layout.location.y;
    }

    fn get_data(&self, tree: &taffy::TaffyTree<()>, urgency: Urgency) -> Vec<Data<'_>> {
        let mut data = self
            .get_instances(tree, urgency)
            .into_iter()
            .map(Data::Instance)
            .chain(
                self.get_text_areas(tree, urgency)
                    .into_iter()
                    .map(Data::TextArea),
            )
            .collect::<Vec<_>>();

        if let Some(progress) = self.progress.as_ref() {
            data.extend(progress.get_data(tree, urgency));
        }

        if let Some(icons) = self.icons.as_ref() {
            data.extend(icons.get_data(tree, urgency));
        }
        if let Some(buttons) = self.buttons.as_ref() {
            data.extend(buttons.get_data(tree));
        }
        if let Some(summary) = self.summary.as_ref() {
            data.extend(summary.get_data(tree, urgency));
        }
        if let Some(body) = self.body.as_ref() {
            data.extend(body.get_data(tree, urgency));
        }

        data
    }

    fn get_node_id(&self) -> taffy::NodeId {
        self.node
    }
}

pub struct Empty;
pub struct Ready;

impl<State> Notification<State> {
    #[must_use]
    pub fn new_empty(
        node_id: taffy::NodeId,
        config: Arc<Config>,
        data: NotificationData,
        ui_state: UiState,
    ) -> Notification<Empty> {
        let context = components::Context {
            id: data.id,
            app_name: Arc::clone(&data.app_name),
            config,
            ui_state,
        };

        Notification {
            node: node_id,
            context,
            summary: None,
            progress: None,
            y: 0.,
            x: 0.,
            icons: None,
            buttons: None,
            data,
            hovered: false,
            registration_token: None,
            body: None,
            _state: std::marker::PhantomData,
        }
    }

    #[must_use]
    pub fn new(
        tree: &mut taffy::TaffyTree,
        config: Arc<Config>,
        font_system: &mut FontSystem,
        data: NotificationData,
        ui_state: UiState,
        sender: Option<calloop::channel::Sender<crate::Event>>,
    ) -> Notification<Ready> {
        let context = components::Context {
            id: data.id,
            app_name: Arc::clone(&data.app_name),
            config,
            ui_state,
        };

        if data.app_name == "next_notification_count".into()
            || data.app_name == "prev_notification_count".into()
        {
            return Notification {
                node: tree.new_leaf(taffy::Style::DEFAULT).unwrap(),
                context,
                y: 0.,
                x: 0.,
                hovered: false,
                icons: None,
                progress: None,
                registration_token: None,
                buttons: None,
                summary: None,
                body: None,
                data,
                _state: std::marker::PhantomData,
            };
        }

        let icons = match (data.hints.image.as_ref(), data.app_icon.as_deref()) {
            (None, None) => None,
            (image, app_icon) => Some(Icons::new(tree, context.clone(), image, app_icon)),
        };

        let mut buttons = ButtonManager::new(context.clone(), data.hints.urgency, sender)
            .add_dismiss(tree, font_system)
            .add_actions(tree, &data.actions, font_system);

        let dismiss_button = buttons
            .buttons()
            .iter()
            .find(|button| button.button_type() == ButtonType::Dismiss)
            .map_or(0.0, |button| button.get_render_bounds(tree).width);

        let style = context.config.find_style(&data.app_name, false);

        let body = if data.body.is_empty() {
            None
        } else {
            let mut body = Body::new(tree, context.clone(), font_system);
            body.set_text(font_system, &data.body);
            body.set_size(
                font_system,
                Some(
                    style.width
                        - icons
                            .as_ref()
                            .map(|icons| icons.get_bounds(tree).width)
                            .unwrap_or_default()
                        - dismiss_button,
                ),
                None,
            );

            buttons = buttons.add_anchors(tree, &body.anchors, font_system);

            Some(body)
        };

        let summary = if data.summary.is_empty() {
            None
        } else {
            let mut summary = Summary::new(tree, context.clone(), font_system);
            summary.set_text(font_system, &data.summary);
            summary.set_size(
                font_system,
                Some(
                    style.width
                        - icons
                            .as_ref()
                            .map(|icons| icons.get_bounds(tree).width)
                            .unwrap_or_default()
                        - dismiss_button,
                ),
                None,
            );

            Some(summary)
        };

        Notification {
            node: tree.new_leaf(taffy::Style::DEFAULT).unwrap(),
            summary,
            progress: data
                .hints
                .value
                .map(|value| Progress::new(tree, context.clone(), value)),
            context,
            y: 0.,
            x: 0.,
            icons,
            buttons: Some(buttons.finish(tree, font_system)),
            data,
            hovered: false,
            registration_token: None,
            body,
            _state: std::marker::PhantomData,
        }
    }

    pub fn replace(
        &mut self,
        tree: &mut taffy::TaffyTree<()>,
        font_system: &mut FontSystem,
        data: NotificationData,
        sender: Option<calloop::channel::Sender<crate::Event>>,
    ) {
        match (
            self.progress.as_mut(),
            data.hints.value,
            self.data.hints.value == data.hints.value,
        ) {
            (Some(progress), Some(value), false) => progress.set_value(value),
            (None, Some(value), _) => {
                self.progress = Some(Progress::new(tree, self.context.clone(), value));
            }
            _ => {}
        }

        match (self.body.as_mut(), self.data.body == data.body) {
            (Some(body), false) => body.set_text(font_system, &data.body),
            (None, _) => {
                self.body = Some(Body::new(tree, self.context.clone(), font_system));
            }
            _ => {}
        }

        if self.data.actions != data.actions || self.data.body != data.body {
            let mut buttons = ButtonManager::new(self.context.clone(), self.urgency(), sender)
                .add_dismiss(tree, font_system)
                .add_actions(tree, &data.actions, font_system);

            if let Some(body) = &self.body {
                buttons = buttons.add_anchors(tree, &body.anchors, font_system);
            }

            self.buttons = Some(buttons.finish(tree, font_system));
        }

        match (self.summary.as_mut(), self.data.summary == data.summary) {
            (Some(summary), false) => summary.set_text(font_system, &data.summary),
            (None, _) => {
                self.summary = Some(Summary::new(tree, self.context.clone(), font_system));
            }
            _ => {}
        }

        self.data = data;
    }

    pub fn start_timer(&mut self, loop_handle: &LoopHandle<'static, Moxnotify>) {
        if let Some(timeout) = self.timeout()
            && self.registration_token.is_none()
        {
            log::debug!(
                "Expiration timer started for notification, id: {}, timeout: {timeout}",
                self.id(),
            );

            let timer = Timer::from_duration(Duration::from_millis(timeout));
            let id = self.id();
            self.registration_token = loop_handle
                .insert_source(timer, move |_, (), moxnotify| {
                    moxnotify.dismiss_by_id(id, Some(Reason::Expired));

                    let loop_handle = moxnotify.loop_handle.clone();
                    moxnotify
                        .notifications
                        .iter_viewed_mut()
                        .for_each(|notification| notification.start_timer(&loop_handle));

                    TimeoutAction::Drop
                })
                .ok();
        }
    }

    pub fn stop_timer(&mut self, loop_handle: &LoopHandle<'static, Moxnotify>) {
        if let Some(token) = self.registration_token.take() {
            log::debug!(
                "Expiration timer paused for notification, id: {}",
                self.id()
            );

            loop_handle.remove(token);
        }
    }

    #[must_use]
    pub fn timeout(&self) -> Option<u64> {
        let notification_style_entry = self
            .context
            .config
            .styles
            .notification
            .iter()
            .find(|entry| entry.app == self.data.app_name);

        let ignore_timeout = notification_style_entry
            .and_then(|entry| entry.ignore_timeout)
            .unwrap_or(self.context.config.general.ignore_timeout);

        let default_timeout = notification_style_entry
            .and_then(|entry| entry.default_timeout.as_ref())
            .unwrap_or(&self.context.config.general.default_timeout);

        if ignore_timeout {
            (default_timeout.get(self.data.hints.urgency) > 0)
                .then(|| (default_timeout.get(self.data.hints.urgency) as u64) * 1000)
        } else {
            match self.data.timeout {
                0 => None,
                -1 => (default_timeout.get(self.data.hints.urgency) > 0)
                    .then(|| (default_timeout.get(self.data.hints.urgency) as u64) * 1000),
                t if t > 0 => Some(t as u64),
                _ => None,
            }
        }
    }

    #[must_use]
    pub fn width(&self) -> f32 {
        if self.hovered() {
            self.context.config.styles.hover.width.resolve(0.)
        } else {
            self.context.config.styles.default.width.resolve(0.)
        }
    }

    #[must_use]
    pub fn urgency(&self) -> Urgency {
        self.data.hints.urgency
    }

    #[must_use]
    pub fn hovered(&self) -> bool {
        self.hovered
    }

    pub fn hover(&mut self) {
        self.hovered = true;
    }

    pub fn unhover(&mut self) {
        self.hovered = false;
    }

    #[must_use]
    pub fn id(&self) -> NotificationId {
        self.data.id
    }
}

impl Notification<Empty> {
    #[must_use]
    pub fn promote(
        self,
        tree: &mut taffy::TaffyTree<()>,
        font_system: &mut FontSystem,
        sender: Option<calloop::channel::Sender<crate::Event>>,
    ) -> Notification<Ready> {
        let icons = match (
            self.data.hints.image.as_ref(),
            self.data.app_icon.as_deref(),
        ) {
            (None, None) => None,
            (image, app_icon) => Some(Icons::new(tree, self.context.clone(), image, app_icon)),
        };

        let mut buttons = ButtonManager::new(self.context.clone(), self.data.hints.urgency, sender)
            .add_dismiss(tree, font_system)
            .add_actions(tree, &self.data.actions, font_system);

        let dismiss_button = buttons
            .buttons()
            .iter()
            .find(|button| button.button_type() == ButtonType::Dismiss)
            .map_or(0.0, |button| button.get_render_bounds(tree).width);

        let style = self.context.config.find_style(&self.data.app_name, false);

        let body = if self.data.body.is_empty() {
            None
        } else {
            let mut body = Body::new(tree, self.context.clone(), font_system);
            body.set_text(font_system, &self.data.body);
            body.set_size(
                font_system,
                Some(
                    style.width
                        - icons
                            .as_ref()
                            .map(|icons| icons.get_bounds(tree).width)
                            .unwrap_or_default()
                        - dismiss_button,
                ),
                None,
            );

            buttons = buttons.add_anchors(tree, &body.anchors, font_system);

            Some(body)
        };

        let summary = if self.data.summary.is_empty() {
            None
        } else {
            let mut summary = Summary::new(tree, self.context.clone(), font_system);
            summary.set_text(font_system, &self.data.summary);
            summary.set_size(
                font_system,
                Some(
                    style.width
                        - icons
                            .as_ref()
                            .map(|icons| icons.get_bounds(tree).width)
                            .unwrap_or_default()
                        - dismiss_button,
                ),
                None,
            );

            Some(summary)
        };

        log::debug!("Notification id: {} loaded", self.id());

        Notification {
            summary,
            progress: self
                .data
                .hints
                .value
                .map(|value| Progress::new(tree, self.context.clone(), value)),
            y: 0.,
            x: 0.,
            icons,
            buttons: Some(buttons.finish(tree, font_system)),
            data: self.data,
            hovered: false,
            registration_token: self.registration_token,
            body,
            context: self.context,
            node: self.node,
            _state: std::marker::PhantomData,
        }
    }
}
