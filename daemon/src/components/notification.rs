use super::button::{ButtonManager, ButtonType, Finished};
use super::icons::Icons;
use super::progress::Progress;
use super::text::body::Body;
use super::text::summary::Summary;
use super::text::Text;
use super::{Bounds, UiState};
use crate::manager::Reason;
use crate::rendering::texture_renderer;
use crate::{
    components::{Component, Data},
    config::{Size, StyleState},
    utils::buffers,
    Config, Moxnotify, NotificationData, Urgency,
};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{LoopHandle, RegistrationToken};
use glyphon::FontSystem;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

pub type NotificationId = u32;

pub struct Notification {
    pub y: f32,
    pub x: f32,
    hovered: bool,
    config: Arc<Config>,
    pub icons: Option<Icons>,
    progress: Option<Progress>,
    pub registration_token: Option<RegistrationToken>,
    pub buttons: ButtonManager<Finished>,
    pub data: NotificationData,
    ui_state: UiState,
    pub summary: Summary,
    pub body: Body,
}

impl PartialEq for Notification {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Component for Notification {
    type Style = StyleState;

    fn get_config(&self) -> &Config {
        &self.config
    }

    fn get_app_name(&self) -> &str {
        &self.data.app_name
    }

    fn get_id(&self) -> u32 {
        self.data.id
    }

    fn get_ui_state(&self) -> &UiState {
        &self.ui_state
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

    fn get_instances(&self, urgency: &Urgency) -> Vec<buffers::Instance> {
        let extents = self.get_render_bounds();
        let style = self.get_style();

        vec![buffers::Instance {
            rect_pos: [extents.x, extents.y],
            rect_size: [
                extents.width - style.border.size.left - style.border.size.right,
                extents.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: self.ui_state.scale.load(Ordering::Relaxed),
            depth: 0.9,
        }]
    }

    fn get_text_areas(&self, _: &Urgency) -> Vec<glyphon::TextArea<'_>> {
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
        let style = self.config.find_style(&self.data.app_name, hovered);

        let x_offset = style.border.size.left + style.padding.left;
        let y_offset = style.border.size.top + style.padding.top;

        // Get action buttons for reuse
        let action_buttons_count = self
            .buttons
            .buttons()
            .iter()
            .filter(|button| button.button_type() == ButtonType::Action)
            .count();

        let max_action_button_height = self
            .buttons
            .buttons()
            .iter()
            .filter(|button| button.button_type() == ButtonType::Action)
            .map(|button| button.get_bounds().height)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or_default();

        // Position icons
        if let Some(icons) = self.icons.as_mut() {
            let progress_height = self
                .progress
                .as_ref()
                .map(|p| p.get_bounds().height)
                .unwrap_or_default();

            let available_height = extents.height
                - style.border.size.top
                - style.border.size.bottom
                - style.padding.top
                - style.padding.bottom
                - progress_height
                - max_action_button_height;

            let vertical_offset = (available_height - self.config.general.icon_size as f32) / 2.0;
            let icon_x = extents.x + x_offset;
            let icon_y = extents.y + y_offset + vertical_offset;

            icons.set_position(icon_x, icon_y);
        }

        // Position summary
        self.summary.set_position(
            extents.x
                + x_offset
                + self
                    .icons
                    .as_ref()
                    .map(|icons| icons.get_bounds().width)
                    .unwrap_or_default(),
            extents.y + y_offset,
        );

        // Position progress indicator if present
        if let Some(progress) = self.progress.as_mut() {
            let available_width = extents.width
                - style.border.size.left
                - style.border.size.right
                - style.padding.left
                - style.padding.right
                - style.progress.margin.left
                - style.progress.margin.right;

            progress.set_width(available_width);

            let is_selected = self.ui_state.selected.load(Ordering::Relaxed)
                && self.ui_state.selected_id.load(Ordering::Relaxed) == self.data.id;
            let selected_style = self.config.find_style(&self.data.app_name, is_selected);

            let progress_x =
                extents.x + selected_style.border.size.left + selected_style.padding.left;
            let progress_y = extents.y + extents.height
                - selected_style.border.size.bottom
                - selected_style.padding.bottom
                - progress.get_bounds().height;

            progress.set_position(progress_x, progress_y);
        }

        let dismiss_bottom_y = self
            .buttons
            .buttons_mut()
            .iter_mut()
            .find(|button| button.button_type() == ButtonType::Dismiss)
            .map(|button| {
                let dismiss_x = extents.x + extents.width
                    - style.border.size.right
                    - style.padding.right
                    - button.get_bounds().width;

                let dismiss_y =
                    extents.y + style.margin.top + style.border.size.top + style.padding.top;

                button.set_position(dismiss_x, dismiss_y);
                button.get_bounds().y + button.get_bounds().height
            })
            .unwrap_or(0.0);

        // Position action buttons
        if action_buttons_count > 0 {
            let button_style = self
                .buttons
                .buttons()
                .iter()
                .find(|button| button.button_type() == ButtonType::Action)
                .map(|button| button.get_style())
                .unwrap_or_else(|| &style.buttons.action.default);

            let side_padding = style.border.size.left
                + style.border.size.right
                + style.padding.left
                + style.padding.right;

            let button_margin = button_style.margin.left + button_style.margin.right;
            let available_width = extents.width - side_padding - button_margin;

            let action_buttons_f32 = action_buttons_count as f32;
            let total_spacing = (action_buttons_f32 - 1.0) * button_margin;
            let button_width = (available_width - total_spacing) / action_buttons_f32;

            self.buttons.set_action_widths(button_width);

            let progress_height = self
                .progress
                .as_ref()
                .map(|p| p.get_bounds().height)
                .unwrap_or_default();

            let base_x = extents.x + style.border.size.left + style.padding.left;
            let bottom_padding = style.border.size.bottom + style.padding.bottom + progress_height;

            self.buttons
                .buttons_mut()
                .iter_mut()
                .filter(|b| b.button_type() == ButtonType::Action)
                .enumerate()
                .for_each(|(i, button)| {
                    let x_position = base_x + (button_width + button_margin) * i as f32;
                    let y_position =
                        (extents.y + extents.height - bottom_padding - button.get_bounds().height)
                            .max(dismiss_bottom_y);

                    button.set_position(x_position, y_position);
                });
        }

        // Position anchor buttons
        self.buttons
            .buttons_mut()
            .iter_mut()
            .filter(|b| b.button_type() == ButtonType::Anchor)
            .for_each(|button| {
                button.set_position(
                    self.body.get_render_bounds().x,
                    self.body.get_render_bounds().y,
                )
            });

        // Position body
        let bounds = self.get_render_bounds();
        self.body.set_position(
            bounds.x
                + x_offset
                + self
                    .icons
                    .as_ref()
                    .map(|icons| icons.get_bounds().width)
                    .unwrap_or_default(),
            bounds.y + y_offset + self.summary.get_bounds().height,
        );
    }

    fn get_data(&self, urgency: &Urgency) -> Vec<Data<'_>> {
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
        data.extend(self.buttons.data());
        data.extend(self.summary.get_data(urgency));
        data.extend(self.body.get_data(urgency));

        data
    }
}

impl Notification {
    pub fn new_empty() -> Self {
        let mut font_system = FontSystem::new();
        let config = Arc::new(Config::default());

        Self {
            summary: Summary::new(
                0,
                Arc::new(Config::default()),
                "".into(),
                UiState::default(),
                &mut font_system,
            ),
            progress: None,
            y: 0.,
            x: 0.,
            icons: None,
            buttons: ButtonManager::new(
                0,
                Urgency::Low,
                "".into(),
                UiState::default(),
                None,
                Arc::clone(&config),
            )
            .add_dismiss(&mut font_system)
            .finish(&mut font_system),
            data: NotificationData::default(),
            config: Arc::clone(&config),
            hovered: false,
            registration_token: None,
            ui_state: UiState::default(),
            body: Body::new(
                0,
                Arc::clone(&config),
                "".into(),
                UiState::default(),
                &mut font_system,
            ),
        }
    }

    pub fn new(
        config: Arc<Config>,
        font_system: &mut FontSystem,
        data: NotificationData,
        ui_state: UiState,
        sender: Option<calloop::channel::Sender<crate::Event>>,
    ) -> Self {
        let mut body = Body::new(
            data.id,
            Arc::clone(&config),
            Arc::clone(&data.app_name),
            ui_state.clone(),
            font_system,
        );

        let mut summary = Summary::new(
            data.id,
            Arc::clone(&config),
            Arc::clone(&data.app_name),
            ui_state.clone(),
            font_system,
        );

        if data.app_name == "next_notification_count".into()
            || data.app_name == "prev_notification_count".into()
        {
            return Self {
                y: 0.,
                x: 0.,
                hovered: false,
                config: Arc::clone(&config),
                icons: None,
                progress: None,
                registration_token: None,
                buttons: ButtonManager::new(
                    data.id,
                    data.hints.urgency,
                    Arc::clone(&data.app_name),
                    ui_state.clone(),
                    sender,
                    Arc::clone(&config),
                )
                .add_dismiss(font_system)
                .finish(font_system),
                ui_state: ui_state.clone(),
                summary,
                body,
                data,
            };
        }

        let icons = match (data.hints.image.as_ref(), data.app_icon.as_deref()) {
            (None, None) => None,
            (image, app_icon) => Some(Icons::new(
                data.id,
                image,
                app_icon,
                Arc::clone(&config),
                ui_state.clone(),
                Arc::clone(&data.app_name),
            )),
        };

        let buttons = ButtonManager::new(
            data.id,
            data.hints.urgency,
            Arc::clone(&data.app_name),
            ui_state.clone(),
            sender,
            Arc::clone(&config),
        )
        .add_dismiss(font_system)
        .add_actions(&data.actions, font_system);

        body.set_text(font_system, &data.body);
        summary.set_text(font_system, &data.summary);

        let dismiss_button = buttons
            .buttons()
            .iter()
            .find(|button| button.button_type() == ButtonType::Dismiss)
            .map(|button| button.get_render_bounds().width)
            .unwrap_or(0.0);

        let style = config.find_style(&data.app_name, false);
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

        Self {
            summary,
            progress: data.hints.value.map(|value| {
                Progress::new(
                    data.id,
                    value,
                    ui_state.clone(),
                    Arc::clone(&config),
                    Arc::clone(&data.app_name),
                )
            }),
            y: 0.,
            x: 0.,
            icons,
            buttons: buttons
                .add_anchors(&body.anchors, font_system)
                .finish(font_system),
            data,
            config,
            hovered: false,
            registration_token: None,
            ui_state: ui_state.clone(),
            body,
        }
    }

    pub fn start_timer(&mut self, loop_handle: &LoopHandle<'static, Moxnotify>) {
        if let Some(timeout) = self.timeout() {
            log::debug!(
                "Expiration timer started for notification, id: {}, timeout: {}",
                self.id(),
                timeout
            );

            let timer = Timer::from_duration(Duration::from_millis(timeout));
            let id = self.id();
            self.registration_token = loop_handle
                .insert_source(timer, move |_, _, moxnotify| {
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

    pub fn timeout(&self) -> Option<u64> {
        let notification_style_entry = self
            .config
            .styles
            .notification
            .iter()
            .find(|entry| entry.app == self.data.app_name);

        let ignore_timeout = notification_style_entry
            .and_then(|entry| entry.ignore_timeout)
            .unwrap_or(self.config.general.ignore_timeout);

        let default_timeout = notification_style_entry
            .and_then(|entry| entry.default_timeout.as_ref())
            .unwrap_or(&self.config.general.default_timeout);

        if ignore_timeout {
            (default_timeout.get(&self.data.hints.urgency) > 0)
                .then(|| (default_timeout.get(&self.data.hints.urgency) as u64) * 1000)
        } else {
            match self.data.timeout {
                0 => None,
                -1 => (default_timeout.get(&self.data.hints.urgency) > 0)
                    .then(|| (default_timeout.get(&self.data.hints.urgency) as u64) * 1000),
                t if t > 0 => Some(t as u64),
                _ => None,
            }
        }
    }

    pub fn height(&self) -> f32 {
        let style = self.get_style();

        let dismiss_button = self
            .buttons
            .buttons()
            .iter()
            .find(|button| button.button_type() == ButtonType::Dismiss)
            .map(|b| b.get_bounds().height)
            .unwrap_or(0.0);

        let action_button = self
            .buttons
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
                let text_height =
                    self.body.get_bounds().height + self.summary.get_bounds().height + progress;
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

    pub fn width(&self) -> f32 {
        match self.hovered() {
            true => self.config.styles.hover.width.resolve(0.),
            false => self.config.styles.default.width.resolve(0.),
        }
    }

    pub fn urgency(&self) -> &Urgency {
        &self.data.hints.urgency
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    pub fn hover(&mut self) {
        self.hovered = true;
    }

    pub fn unhover(&mut self) {
        self.hovered = false;
    }

    pub fn id(&self) -> NotificationId {
        self.data.id
    }
}
