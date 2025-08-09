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
    config::{Size, StyleState},
    utils::buffers,
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
use taffy::{
    TaffyTree,
    style_helpers::{auto, flex, length, line, span},
};

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
            Self::Empty(n) => n.start_timer(loop_handle),
            Self::Ready(n) => n.start_timer(loop_handle),
        }
    }

    pub fn stop_timer(&self, loop_handle: &LoopHandle<'static, Moxnotify>) {
        match self {
            Self::Empty(n) => n.stop_timer(loop_handle),
            Self::Ready(n) => n.stop_timer(loop_handle),
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        match self {
            Self::Empty(_) => unreachable!(),
            Self::Ready(n) => n.set_position(x, y),
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
    pub fn get_bounds(&self) -> Bounds {
        match self {
            Self::Empty(_) => {
                unreachable!()
            }
            Self::Ready(n) => n.get_bounds(),
        }
    }

    pub fn get_render_bounds(&self) -> Bounds {
        match self {
            Self::Empty(_) => {
                unreachable!()
            }
            Self::Ready(n) => n.get_render_bounds(),
        }
    }

    pub fn unhover(&mut self) {
        match self {
            Self::Empty(_) => {
                unreachable!()
            }
            Self::Ready(n) => n.unhover(),
        }
    }

    pub fn replace(
        &mut self,
        font_system: &mut FontSystem,
        data: NotificationData,
        sender: Option<calloop::channel::Sender<crate::Event>>,
    ) {
        match self {
            Self::Empty(n) => n.replace(font_system, data, sender),
            Self::Ready(n) => n.replace(font_system, data, sender),
        }
    }

    #[must_use]
    pub fn buttons(&self) -> Option<&ButtonManager<Finished>> {
        match self {
            Self::Empty(n) => n.buttons.as_ref(),
            Self::Ready(n) => n.buttons.as_ref(),
        }
    }

    pub fn buttons_mut(&mut self) -> Option<&mut ButtonManager<Finished>> {
        match self {
            Self::Empty(n) => n.buttons.as_mut(),
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

    fn get_bounds(&self) -> Bounds {
        let style = self.get_style();

        Bounds {
            x: 0.,
            y: self.y,
            width: self.width()
                + style.border.size.left
                + style.border.size.right
                + style.padding.left
                + style.padding.right
                + style.margin.left
                + style.margin.right,
            height: self.height()
                + style.border.size.top
                + style.border.size.bottom
                + style.padding.top
                + style.padding.bottom
                + style.margin.top
                + style.margin.bottom,
        }
    }

    fn get_render_bounds(&self) -> Bounds {
        let extents = self.get_bounds();
        let style = self.get_style();

        Bounds {
            x: extents.x + style.margin.left + self.x + self.data.hints.x as f32,
            y: extents.y + style.margin.top,
            width: extents.width - style.margin.left - style.margin.right,
            height: extents.height - style.margin.top - style.margin.bottom,
        }
    }

    fn get_instances(&self, urgency: Urgency) -> Vec<buffers::Instance> {
        let extents = self.get_render_bounds();
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

    fn get_text_areas(&self, _: Urgency) -> Vec<glyphon::TextArea<'_>> {
        Vec::new()
    }

    fn get_textures(&self) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;

        let extents = self.get_render_bounds();
        let hovered = self.hovered();
        let style = self.context.config.find_style(&self.data.app_name, hovered);

        let action_buttons_count = self
            .buttons
            .as_ref()
            .map(|buttons| {
                buttons
                    .buttons()
                    .iter()
                    .filter(|button| button.button_type() == ButtonType::Action)
                    .count()
            })
            .unwrap_or_default();

        let mut tree: TaffyTree<()> = TaffyTree::new();

        let icons_size = self
            .icons
            .as_ref()
            .map(|i| i.get_bounds())
            .unwrap_or_default();
        let summary_size = self
            .summary
            .as_ref()
            .map(|s| s.get_bounds())
            .unwrap_or_default();
        let progress_size = self
            .progress
            .as_ref()
            .map(|p| p.get_bounds())
            .unwrap_or_default();

        let dismiss_size = self
            .buttons
            .as_ref()
            .and_then(|buttons| {
                buttons
                    .buttons()
                    .iter()
                    .find(|button| button.button_type() == ButtonType::Dismiss)
                    .map(|button| button.get_bounds())
            })
            .unwrap_or_default();

        let action_buttons_height = if action_buttons_count > 0 {
            self.buttons
                .as_ref()
                .and_then(|buttons| {
                    buttons
                        .buttons()
                        .iter()
                        .filter(|b| b.button_type() == ButtonType::Action)
                        .map(|b| b.get_bounds().height)
                        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                })
                .unwrap_or(0.0)
        } else {
            0.0
        };

        let container_node = tree
            .new_leaf(taffy::Style {
                size: taffy::Size {
                    width: length(extents.width),
                    height: length(extents.height),
                },
                padding: taffy::Rect {
                    left: length(style.padding.left.resolve(0.)),
                    right: length(style.padding.right.resolve(0.)),
                    top: length(style.padding.top.resolve(0.)),
                    bottom: length(style.padding.bottom.resolve(0.)),
                },
                display: taffy::Display::Grid,
                grid_template_rows: vec![
                    length(summary_size.height.max(dismiss_size.height)),
                    auto(),
                    length(action_buttons_height),
                    length(progress_size.height),
                ],
                grid_template_columns: vec![
                    length(icons_size.width),
                    flex(1.0),
                    length(dismiss_size.width),
                ],
                ..Default::default()
            })
            .unwrap();

        // Icons node
        let icons_node = if self.icons.is_some() {
            let node = tree
                .new_leaf(taffy::Style {
                    grid_row: span(2),
                    grid_column: line(1),
                    size: taffy::Size {
                        width: length(icons_size.width),
                        height: length(icons_size.height),
                    },
                    ..Default::default()
                })
                .unwrap();
            tree.add_child(container_node, node).unwrap();
            Some(node)
        } else {
            None
        };

        let summary_node = if self.summary.is_some() {
            let node = tree
                .new_leaf(taffy::Style {
                    grid_row: line(1),
                    grid_column: line(2),
                    size: taffy::Size {
                        width: auto(),
                        height: length(summary_size.height),
                    },
                    ..Default::default()
                })
                .unwrap();
            tree.add_child(container_node, node).unwrap();
            Some(node)
        } else {
            None
        };

        let dismiss_node = tree
            .new_leaf(taffy::Style {
                grid_row: line(1),
                grid_column: line(3),
                size: taffy::Size {
                    width: length(dismiss_size.width),
                    height: length(dismiss_size.height),
                },
                ..Default::default()
            })
            .unwrap();
        tree.add_child(container_node, dismiss_node).unwrap();

        let body_node = if self.body.is_some() {
            let node = tree
                .new_leaf(taffy::Style {
                    grid_row: line(2),
                    grid_column: line(2),
                    size: taffy::Size {
                        width: auto(),
                        height: auto(),
                    },
                    flex_grow: 1.0,
                    ..Default::default()
                })
                .unwrap();
            tree.add_child(container_node, node).unwrap();
            Some(node)
        } else {
            None
        };

        let action_buttons_node = if action_buttons_count > 0 {
            let node = tree
                .new_leaf(taffy::Style {
                    grid_row: line(3),
                    grid_column: span(3),
                    display: taffy::Display::Flex,
                    flex_direction: taffy::FlexDirection::Row,
                    justify_content: Some(taffy::JustifyContent::SpaceBetween),
                    size: taffy::Size {
                        width: auto(),
                        height: length(action_buttons_height),
                    },
                    ..Default::default()
                })
                .unwrap();
            tree.add_child(container_node, node).unwrap();
            Some(node)
        } else {
            None
        };

        let progress_node = if self.progress.is_some() {
            let node = tree
                .new_leaf(taffy::Style {
                    grid_row: line(4),
                    grid_column: span(3),
                    size: taffy::Size {
                        width: auto(),
                        height: length(progress_size.height),
                    },
                    ..Default::default()
                })
                .unwrap();
            tree.add_child(container_node, node).unwrap();
            Some(node)
        } else {
            None
        };

        tree.compute_layout(
            container_node,
            taffy::Size {
                width: taffy::AvailableSpace::Definite(extents.width),
                height: taffy::AvailableSpace::MinContent,
            },
        )
        .unwrap();

        if let Some(icons) = icons_node {
            let res = tree.layout(icons).unwrap();
            self.icons
                .as_mut()
                .unwrap()
                .set_position(res.location.x, res.location.y);
        }

        if let Some(summary) = summary_node {
            let res = tree.layout(summary).unwrap();
            self.summary
                .as_mut()
                .unwrap()
                .set_position(res.location.x, res.location.y);
        }

        let res = tree.layout(dismiss_node).unwrap();
        if let Some(buttons) = self.buttons.as_mut() {
            if let Some(dismiss_button) = buttons
                .buttons_mut()
                .iter_mut()
                .find(|button| button.button_type() == ButtonType::Dismiss)
            {
                dismiss_button.set_position(res.location.x, res.location.y);
            }
        }

        if let Some(body) = body_node {
            let res = tree.layout(body).unwrap();
            self.body
                .as_mut()
                .unwrap()
                .set_position(res.location.x, res.location.y);
        }

        if let Some(actions) = action_buttons_node {
            let res = tree.layout(actions).unwrap();

            if let Some(buttons) = self.buttons.as_mut() {
                let action_buttons: Vec<_> = buttons
                    .buttons_mut()
                    .iter_mut()
                    .filter(|b| b.button_type() == ButtonType::Action)
                    .collect();

                let button_spacing = if action_buttons.len() > 1 {
                    (res.size.width
                        - action_buttons
                            .iter()
                            .map(|b| b.get_bounds().width)
                            .sum::<f32>())
                        / (action_buttons.len() - 1) as f32
                } else {
                    0.0
                };

                let mut current_x = res.location.x;
                for button in action_buttons {
                    button.set_position(current_x, res.location.y);
                    current_x += button.get_bounds().width + button_spacing;
                }
            }
        }

        if let Some(progress) = progress_node {
            let res = tree.layout(progress).unwrap();
            self.progress
                .as_mut()
                .unwrap()
                .set_position(res.location.x, res.location.y);
            self.progress.as_mut().unwrap().set_width(res.size.width);
        }
    }

    fn get_data(&self, urgency: Urgency) -> Vec<Data<'_>> {
        let mut data = self
            .get_instances(urgency)
            .into_iter()
            .map(Data::Instance)
            .chain(self.get_text_areas(urgency).into_iter().map(Data::TextArea))
            .collect::<Vec<_>>();

        if let Some(progress) = self.progress.as_ref() {
            data.extend(progress.get_data(urgency));
        }

        if let Some(icons) = self.icons.as_ref() {
            data.extend(icons.get_data(urgency));
        }
        if let Some(buttons) = self.buttons.as_ref() {
            data.extend(buttons.get_data());
        }
        if let Some(summary) = self.summary.as_ref() {
            data.extend(summary.get_data(urgency));
        }
        if let Some(body) = self.body.as_ref() {
            data.extend(body.get_data(urgency));
        }

        data
    }
}

pub struct Empty;
pub struct Ready;

impl<State> Notification<State> {
    #[must_use]
    pub fn new_empty(
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
            (image, app_icon) => Some(Icons::new(context.clone(), image, app_icon)),
        };

        let mut buttons = ButtonManager::new(context.clone(), data.hints.urgency, sender)
            .add_dismiss(font_system)
            .add_actions(&data.actions, font_system);

        let dismiss_button = buttons
            .buttons()
            .iter()
            .find(|button| button.button_type() == ButtonType::Dismiss)
            .map_or(0.0, |button| button.get_render_bounds().width);

        let style = context.config.find_style(&data.app_name, false);

        let body = if data.body.is_empty() {
            None
        } else {
            let mut body = Body::new(context.clone(), font_system);
            body.set_text(font_system, &data.body);
            body.set_size(
                font_system,
                Some(
                    style.width
                        - icons
                            .as_ref()
                            .map(|icons| icons.get_bounds().width)
                            .unwrap_or_default()
                        - dismiss_button,
                ),
                None,
            );

            buttons = buttons.add_anchors(&body.anchors, font_system);

            Some(body)
        };

        let summary = if data.summary.is_empty() {
            None
        } else {
            let mut summary = Summary::new(context.clone(), font_system);
            summary.set_text(font_system, &data.summary);
            summary.set_size(
                font_system,
                Some(
                    style.width
                        - icons
                            .as_ref()
                            .map(|icons| icons.get_bounds().width)
                            .unwrap_or_default()
                        - dismiss_button,
                ),
                None,
            );

            Some(summary)
        };

        Notification {
            summary,
            progress: data
                .hints
                .value
                .map(|value| Progress::new(context.clone(), value)),
            context,
            y: 0.,
            x: 0.,
            icons,
            buttons: Some(buttons.finish(font_system)),
            data,
            hovered: false,
            registration_token: None,
            body,
            _state: std::marker::PhantomData,
        }
    }

    pub fn replace(
        &mut self,
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
                self.progress = Some(Progress::new(self.context.clone(), value));
            }
            _ => {}
        }

        match (self.body.as_mut(), self.data.body == data.body) {
            (Some(body), false) => body.set_text(font_system, &data.body),
            (None, _) => {
                self.body = Some(Body::new(self.context.clone(), font_system));
            }
            _ => {}
        }

        if self.data.actions != data.actions || self.data.body != data.body {
            let mut buttons = ButtonManager::new(self.context.clone(), self.urgency(), sender)
                .add_dismiss(font_system)
                .add_actions(&data.actions, font_system);

            if let Some(body) = &self.body {
                buttons = buttons.add_anchors(&body.anchors, font_system);
            }

            self.buttons = Some(buttons.finish(font_system));
        }

        match (self.summary.as_mut(), self.data.summary == data.summary) {
            (Some(summary), false) => summary.set_text(font_system, &data.summary),
            (None, _) => {
                self.summary = Some(Summary::new(self.context.clone(), font_system));
            }
            _ => {}
        }

        self.data = data;
    }

    pub fn start_timer(&mut self, loop_handle: &LoopHandle<'static, Moxnotify>) {
        self.stop_timer(loop_handle);

        if let Some(timeout) = self.timeout() {
            log::debug!(
                "Expiration timer started for notification, id: {}, timeout: {}",
                self.id(),
                timeout
            );

            let timer = Timer::from_duration(Duration::from_millis(timeout));
            let id = self.id();
            self.registration_token = loop_handle
                .insert_source(timer, move |_, (), moxnotify| {
                    moxnotify.dismiss_by_id(id, Some(Reason::Expired));
                    TimeoutAction::Drop
                })
                .ok();
        }
    }

    pub fn stop_timer(&self, loop_handle: &LoopHandle<'static, Moxnotify>) {
        if let Some(token) = self.registration_token {
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
        font_system: &mut FontSystem,
        sender: Option<calloop::channel::Sender<crate::Event>>,
    ) -> Notification<Ready> {
        let icons = match (
            self.data.hints.image.as_ref(),
            self.data.app_icon.as_deref(),
        ) {
            (None, None) => None,
            (image, app_icon) => Some(Icons::new(self.context.clone(), image, app_icon)),
        };

        let mut buttons = ButtonManager::new(self.context.clone(), self.data.hints.urgency, sender)
            .add_dismiss(font_system)
            .add_actions(&self.data.actions, font_system);

        let dismiss_button = buttons
            .buttons()
            .iter()
            .find(|button| button.button_type() == ButtonType::Dismiss)
            .map_or(0.0, |button| button.get_render_bounds().width);

        let style = self.context.config.find_style(&self.data.app_name, false);

        let body = if self.data.body.is_empty() {
            None
        } else {
            let mut body = Body::new(self.context.clone(), font_system);
            body.set_text(font_system, &self.data.body);
            body.set_size(
                font_system,
                Some(
                    style.width
                        - icons
                            .as_ref()
                            .map(|icons| icons.get_bounds().width)
                            .unwrap_or_default()
                        - dismiss_button,
                ),
                None,
            );

            buttons = buttons.add_anchors(&body.anchors, font_system);

            Some(body)
        };

        let summary = if self.data.summary.is_empty() {
            None
        } else {
            let mut summary = Summary::new(self.context.clone(), font_system);
            summary.set_text(font_system, &self.data.summary);
            summary.set_size(
                font_system,
                Some(
                    style.width
                        - icons
                            .as_ref()
                            .map(|icons| icons.get_bounds().width)
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
                .map(|value| Progress::new(self.context.clone(), value)),
            y: 0.,
            x: 0.,
            icons,
            buttons: Some(buttons.finish(font_system)),
            data: self.data,
            hovered: false,
            registration_token: self.registration_token,
            body,
            context: self.context,
            _state: std::marker::PhantomData,
        }
    }

    #[must_use]
    pub fn height(&self) -> f32 {
        0.
    }
}

impl Notification<Ready> {
    #[must_use]
    pub fn height(&self) -> f32 {
        let style = self.get_style();

        let dismiss_button = self
            .buttons
            .as_ref()
            .and_then(|buttons| {
                buttons
                    .buttons()
                    .iter()
                    .find(|button| button.button_type() == ButtonType::Dismiss)
                    .map(|b| b.get_bounds().height)
            })
            .unwrap_or_default();

        let action_button = self
            .buttons
            .as_ref()
            .and_then(|buttons| {
                buttons
                    .buttons()
                    .iter()
                    .filter_map(|button| match button.button_type() {
                        ButtonType::Action => Some(button.get_bounds()),
                        _ => None,
                    })
                    .max_by(|a, b| {
                        a.height
                            .partial_cmp(&b.height)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            })
            .unwrap_or_default();

        let progress = if self.progress.is_some() {
            style.progress.height + style.progress.margin.top + style.progress.margin.bottom
        } else {
            0.0
        };

        let min_height = match style.min_height {
            Size::Auto => 0.0,
            Size::Value(value) => value,
        };

        let max_height = match style.max_height {
            Size::Auto => f32::INFINITY,
            Size::Value(value) => value,
        };

        match style.height {
            Size::Value(height) => height.clamp(min_height, max_height),
            Size::Auto => {
                let text_height = self
                    .body
                    .as_ref()
                    .map(|body| body.get_bounds().height)
                    .unwrap_or_default()
                    + self
                        .summary
                        .as_ref()
                        .map(|summary| summary.get_bounds().height)
                        .unwrap_or_default()
                    + progress;
                let icon_height = self
                    .icons
                    .as_ref()
                    .map(|icons| icons.get_bounds().height)
                    .unwrap_or_default()
                    + progress;
                let base_height = (text_height.max(icon_height).max(dismiss_button)
                    + action_button.height)
                    .max(dismiss_button + action_button.height)
                    + style.padding.bottom;
                base_height.clamp(min_height, max_height)
            }
        }
    }
}
