mod action;
mod anchor;
mod dismiss;

use super::text::body;
use crate::{
    Urgency,
    components::{Bounds, Component, Data},
    config::{
        self, Config,
        button::ButtonState,
        keymaps::{self},
    },
    manager::UiState,
    rendering::{text_renderer, texture_renderer},
    utils::buffers,
};
use action::ActionButton;
use anchor::AnchorButton;
use dismiss::DismissButton;
use glyphon::{FontSystem, TextArea};
use std::sync::{Arc, atomic::Ordering};

#[derive(Clone, Copy, Debug)]
pub enum State {
    Unhovered,
    Hovered,
}

pub trait Button: Component + Send + Sync {
    fn hint(&self) -> &Hint;

    fn click(&self);

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    fn button_type(&self) -> ButtonType;

    fn state(&self) -> State;

    fn hover(&mut self);

    fn unhover(&mut self);

    fn set_hint(&mut self, hint: Hint);
}

#[derive(Clone, PartialEq)]
pub enum ButtonType {
    Dismiss,
    Action,
    Anchor,
}

pub struct NotReady;
pub struct Ready;
pub struct Finished;

pub struct ButtonManager<State = NotReady> {
    app_name: Arc<str>,
    id: u32,
    buttons: Vec<Box<dyn Button<Style = ButtonState>>>,
    urgency: Urgency,
    pub ui_state: UiState,
    sender: Option<calloop::channel::Sender<crate::Event>>,
    config: Arc<Config>,
    _state: std::marker::PhantomData<State>,
}

impl ButtonManager<NotReady> {
    pub fn new(
        id: u32,
        urgency: Urgency,
        app_name: Arc<str>,
        ui_state: UiState,
        sender: Option<calloop::channel::Sender<crate::Event>>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            id,
            buttons: Vec::new(),
            urgency,
            ui_state,
            sender,
            config,
            app_name,
            _state: std::marker::PhantomData,
        }
    }

    pub fn add_actions(
        self,
        actions: &[(Arc<str>, Arc<str>)],
        font_system: &mut FontSystem,
    ) -> Self {
        let app_name = Arc::clone(&self.app_name);
        self.internal_add_actions(app_name, actions, font_system)
    }

    pub fn add_anchors(self, anchors: &[Arc<body::Anchor>], font_system: &mut FontSystem) -> Self {
        self.internal_add_anchors(anchors, font_system)
    }

    pub fn add_dismiss(mut self, font_system: &mut FontSystem) -> ButtonManager<Ready> {
        let font = &self.config.styles.default.buttons.dismiss.default.font;
        let text = text_renderer::Text::new(font, font_system, "X");

        let button = DismissButton {
            id: self.id,
            app_name: "".into(),
            ui_state: self.ui_state.clone(),
            hint: Hint::new(
                0,
                "",
                "".into(),
                Arc::clone(&self.config),
                font_system,
                self.ui_state.clone(),
            ),
            text,
            x: 0.,
            y: 0.,
            config: Arc::clone(&self.config),
            state: State::Unhovered,
            tx: self.sender.clone(),
        };

        self.buttons.push(Box::new(button));

        ButtonManager {
            id: self.id,
            app_name: self.app_name,
            buttons: self.buttons,
            urgency: self.urgency,
            ui_state: self.ui_state,
            sender: self.sender,
            config: self.config,
            _state: std::marker::PhantomData,
        }
    }
}

impl ButtonManager<Ready> {
    pub fn add_actions(
        self,
        actions: &[(Arc<str>, Arc<str>)],
        font_system: &mut FontSystem,
    ) -> Self {
        let app_name = Arc::clone(&self.app_name);
        self.internal_add_actions(app_name, actions, font_system)
    }

    pub fn add_anchors(self, anchors: &[Arc<body::Anchor>], font_system: &mut FontSystem) -> Self {
        self.internal_add_anchors(anchors, font_system)
    }

    pub fn finish(mut self, font_system: &mut FontSystem) -> ButtonManager<Finished> {
        let hint_chars: Vec<char> = self.config.general.hint_characters.chars().collect();
        let n = hint_chars.len() as i32;

        self.buttons.iter_mut().enumerate().for_each(|(i, button)| {
            let mut m = i as i32;
            let mut indices = Vec::new();

            loop {
                let rem = (m % n) as usize;
                indices.push(rem);
                m = (m / n) - 1;
                if m < 0 {
                    break;
                }
            }

            indices.reverse();
            let combination: String = indices.into_iter().map(|i| hint_chars[i]).collect();
            let hint = Hint::new(
                0,
                &combination,
                "".into(),
                Arc::clone(&self.config),
                font_system,
                self.ui_state.clone(),
            );

            button.set_hint(hint);
        });

        ButtonManager {
            id: self.id,
            app_name: self.app_name,
            buttons: self.buttons,
            urgency: self.urgency,
            ui_state: self.ui_state,
            sender: self.sender,
            config: self.config,
            _state: std::marker::PhantomData,
        }
    }
}

impl ButtonManager<Finished> {
    pub fn click(&self, x: f64, y: f64) -> bool {
        self.buttons
            .iter()
            .filter_map(|button| {
                let bounds = button.get_render_bounds();
                if x >= bounds.x as f64
                    && y >= bounds.y as f64
                    && x <= (bounds.x + bounds.width) as f64
                    && y <= (bounds.y + bounds.height) as f64
                {
                    button.click();
                    Some(true)
                } else {
                    None
                }
            })
            .next()
            .is_some()
    }

    pub fn hover(&mut self, x: f64, y: f64) -> bool {
        self.buttons
            .iter_mut()
            .filter_map(|button| {
                let bounds = button.get_render_bounds();
                if x >= bounds.x as f64
                    && y >= bounds.y as f64
                    && x <= (bounds.x + bounds.width) as f64
                    && y <= (bounds.y + bounds.height) as f64
                {
                    button.hover();
                    Some(true)
                } else {
                    button.unhover();
                    None
                }
            })
            .next()
            .is_some()
    }

    pub fn hint<T>(&mut self, combination: T)
    where
        T: AsRef<str>,
    {
        if let Some(button) = self
            .buttons
            .iter()
            .find(|button| &*button.hint().combination == combination.as_ref())
        {
            button.click();
        }
    }

    pub fn instances(&self) -> Vec<buffers::Instance> {
        let mut buttons = self
            .buttons
            .iter()
            .flat_map(|button| button.get_instances(&self.urgency))
            .collect::<Vec<_>>();

        if self.ui_state.mode.load(Ordering::Relaxed) == keymaps::Mode::Hint
            && self.ui_state.selected_id.load(Ordering::Relaxed) == self.id
            && self.ui_state.selected.load(Ordering::Relaxed)
        {
            let hints = self
                .buttons
                .iter()
                .flat_map(|button| button.hint().get_instances(&self.urgency))
                .collect::<Vec<_>>();
            buttons.extend_from_slice(&hints);
        }

        buttons
    }

    pub fn text_areas(&self) -> Vec<TextArea<'_>> {
        let mut text_areas = self
            .buttons
            .iter()
            .flat_map(|button| button.get_text_areas(&self.urgency))
            .collect::<Vec<_>>();

        if self.ui_state.mode.load(Ordering::Relaxed) == keymaps::Mode::Hint
            && self.ui_state.selected_id.load(Ordering::Relaxed) == self.id
            && self.ui_state.selected.load(Ordering::Relaxed)
        {
            let hints = self
                .buttons
                .iter()
                .flat_map(|button| button.hint().get_text_areas(&self.urgency));
            text_areas.extend(hints);
        }

        text_areas
    }

    pub fn get_data(&self) -> Vec<Data<'_>> {
        let mut data = self
            .buttons
            .iter()
            .flat_map(|button| button.get_data(&self.urgency))
            .collect::<Vec<_>>();

        if self.ui_state.mode.load(Ordering::Relaxed) == keymaps::Mode::Hint
            && self.ui_state.selected_id.load(Ordering::Relaxed) == self.id
            && self.ui_state.selected.load(Ordering::Relaxed)
        {
            let hints = self
                .buttons
                .iter()
                .flat_map(|button| button.hint().get_data(&self.urgency));
            data.extend(hints);
        }

        data
    }

    pub fn set_action_widths(&mut self, width: f32) {
        self.buttons
            .iter_mut()
            .filter_map(|button| button.as_any_mut().downcast_mut::<ActionButton>())
            .for_each(|action| {
                action.width = width;
            });
    }
}

impl<S> ButtonManager<S> {
    fn internal_add_anchors(
        mut self,
        anchors: &[Arc<body::Anchor>],
        font_system: &mut FontSystem,
    ) -> Self {
        if anchors.is_empty() {
            return self;
        }

        let font = &self.config.styles.default.buttons.action.default.font;

        self.buttons.extend(anchors.iter().map(|anchor| {
            let text = text_renderer::Text::new(font, font_system, "");
            Box::new(AnchorButton {
                id: self.id,
                x: 0.,
                y: 0.,
                hint: Hint::new(
                    0,
                    "",
                    "".into(),
                    Arc::clone(&self.config),
                    font_system,
                    self.ui_state.clone(),
                ),
                config: Arc::clone(&self.config),
                state: State::Unhovered,
                tx: self.sender.clone(),
                text,
                ui_state: self.ui_state.clone(),
                anchor: Arc::clone(anchor),
                app_name: Arc::clone(&self.app_name),
            }) as Box<dyn Button<Style = ButtonState>>
        }));

        self
    }

    fn internal_add_actions(
        mut self,
        app_name: Arc<str>,
        actions: &[(Arc<str>, Arc<str>)],
        font_system: &mut FontSystem,
    ) -> Self {
        if actions.is_empty() {
            return self;
        }

        let mut buttons = actions
            .iter()
            .cloned()
            .map(|action| {
                let font = &self.config.styles.default.buttons.action.default.font;
                let text = text_renderer::Text::new(font, font_system, &action.1);

                Box::new(ActionButton {
                    id: self.id,
                    ui_state: self.ui_state.clone(),
                    hint: Hint::new(
                        0,
                        "",
                        "".into(),
                        Arc::clone(&self.config),
                        font_system,
                        self.ui_state.clone(),
                    ),
                    text,
                    x: 0.,
                    y: 0.,
                    config: Arc::clone(&self.config),
                    action: action.0,
                    state: State::Unhovered,
                    width: 0.,
                    app_name: Arc::clone(&app_name),
                    tx: self.sender.clone(),
                }) as Box<dyn Button<Style = ButtonState>>
            })
            .collect();

        self.buttons.append(&mut buttons);

        self
    }

    pub fn buttons(&self) -> &[Box<dyn Button<Style = ButtonState>>] {
        &self.buttons
    }

    pub fn buttons_mut(&mut self) -> &mut [Box<dyn Button<Style = ButtonState>>] {
        &mut self.buttons
    }
}

pub struct Hint {
    id: u32,
    combination: Box<str>,
    app_name: Arc<str>,
    text: text_renderer::Text,
    config: Arc<Config>,
    ui_state: UiState,
    x: f32,
    y: f32,
}

impl Hint {
    pub fn new<T>(
        id: u32,
        combination: T,
        app_name: Arc<str>,
        config: Arc<Config>,
        font_system: &mut FontSystem,
        ui_state: UiState,
    ) -> Self
    where
        T: AsRef<str>,
    {
        Self {
            id,
            app_name,
            combination: combination.as_ref().into(),
            ui_state,
            text: text_renderer::Text::new(
                &config.styles.default.font,
                font_system,
                combination.as_ref(),
            ),
            config,
            x: 0.,
            y: 0.,
        }
    }
}

impl Component for Hint {
    type Style = config::Hint;

    fn get_config(&self) -> &Config {
        &self.config
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_app_name(&self) -> &str {
        &self.app_name
    }

    fn get_ui_state(&self) -> &UiState {
        &self.ui_state
    }

    fn get_style(&self) -> &Self::Style {
        &self.config.styles.hover.hint
    }

    fn get_bounds(&self) -> Bounds {
        let style = self.get_style();
        let text_extents = self.text.get_bounds();

        let width = style.width.resolve(text_extents.width)
            + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right
            + style.margin.left
            + style.margin.right;

        let height = style.height.resolve(text_extents.height)
            + style.border.size.top
            + style.border.size.bottom
            + style.padding.top
            + style.padding.bottom
            + style.margin.top
            + style.margin.bottom;

        Bounds {
            x: self.x - width / 2.,
            y: self.y - height / 2.,
            width,
            height,
        }
    }

    fn get_render_bounds(&self) -> Bounds {
        let bounds = self.get_bounds();
        let style = self.get_style();

        Bounds {
            x: bounds.x + style.margin.left,
            y: bounds.y + style.margin.top,
            width: bounds.width - style.margin.left - style.margin.right,
            height: bounds.height - style.margin.top - style.margin.bottom,
        }
    }

    fn get_instances(&self, urgency: &Urgency) -> Vec<buffers::Instance> {
        let style = &self.config.styles.hover.hint;
        let bounds = self.get_render_bounds();

        vec![buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: self.ui_state.scale.load(Ordering::Relaxed),
            depth: 0.7,
        }]
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn get_text_areas(&self, urgency: &Urgency) -> Vec<TextArea<'_>> {
        let style = self.get_style();
        let text_extents = self.text.get_bounds();
        let bounds = self.get_render_bounds();

        let remaining_padding = style.width.resolve(text_extents.width) - text_extents.width;
        let (pl, _) = match (style.padding.left.is_auto(), style.padding.right.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.right.resolve(0.)),
            _ => (
                style.padding.left.resolve(0.),
                style.padding.right.resolve(0.),
            ),
        };
        let remaining_padding = style.height.resolve(text_extents.height) - text_extents.height;
        let (pt, _) = match (style.padding.top.is_auto(), style.padding.bottom.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.bottom.resolve(0.)),
            _ => (
                style.padding.top.resolve(0.),
                style.padding.bottom.resolve(0.),
            ),
        };

        vec![TextArea {
            buffer: &self.text.buffer,
            left: bounds.x + style.padding.left.resolve(pl),
            top: bounds.y + style.padding.top.resolve(pt),
            scale: self.ui_state.scale.load(Ordering::Relaxed),
            bounds: glyphon::TextBounds {
                left: (bounds.x + style.padding.left.resolve(pl)) as i32,
                top: (bounds.y + style.padding.top.resolve(pt)) as i32,
                right: (bounds.x + style.padding.left.resolve(pl) + bounds.width) as i32,
                bottom: (bounds.y + style.padding.top.resolve(pt) + bounds.height) as i32,
            },
            default_color: style.font.color.into_glyphon(urgency),
            custom_glyphs: &[],
        }]
    }

    fn get_textures(&self) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ButtonManager;
    use crate::{Urgency, manager::UiState};
    use glyphon::FontSystem;
    use std::sync::Arc;

    #[test]
    fn test_button_click_detection() {
        let config = Arc::new(crate::config::Config::default());
        let ui_state = UiState::default();
        let mut font_system = FontSystem::new();

        let mut button_manager = ButtonManager::new(
            1,
            Urgency::Normal,
            "".into(),
            ui_state,
            None,
            Arc::clone(&config),
        )
        .add_dismiss(&mut font_system)
        .finish(&mut font_system);

        let button = &mut button_manager.buttons_mut()[0];
        button.set_position(10.0, 10.0);

        let style = button.get_style();
        let width = style.width
            + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right;

        let height = style.height
            + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right;

        // Define test points: (x, y, should_click)
        let test_points = [
            // Internal point (should click)
            (10.0 + width as f64 / 2.0, 10.0 + height as f64 / 2.0, true),
            // Exact corners (should click)
            (10.0, 10.0, true),                                // Top left
            (10.0 + width as f64, 10.0, true),                 // Top right
            (10.0, 10.0 + height as f64, true),                // Bottom left
            (10.0 + width as f64, 10.0 + height as f64, true), // Bottom right
            // Just outside corners (should not click)
            (10.0 - 0.1, 10.0, false), // Top left
            (10.0, 10.0 - 0.1, false),
            (10.0 + width as f64 + 0.1, 10.0, false), // Top right
            (10.0 + width as f64, 10.0 - 0.1, false),
            (10.0 - 0.1, 10.0 + height as f64, false), // Bottom left
            (10.0, 10.0 + height as f64 + 0.1, false),
            (10.0 + width as f64 + 0.1, 10.0 + height as f64, false), // Bottom right
            (10.0 + width as f64, 10.0 + height as f64 + 0.1, false),
        ];

        test_points
            .iter()
            .enumerate()
            .for_each(|(i, (x, y, expected))| {
                assert_eq!(
                    button_manager.click(*x, *y),
                    *expected,
                    "Test point {i} at ({x}, {y}) failed",
                );
            });
    }

    #[test]
    fn test_button_hover_detection() {
        let config = Arc::new(crate::config::Config::default());
        let ui_state = UiState::default();
        let mut font_system = FontSystem::new();

        let mut button_manager = ButtonManager::new(
            1,
            Urgency::Normal,
            "".into(),
            ui_state,
            None,
            Arc::clone(&config),
        )
        .add_dismiss(&mut font_system)
        .finish(&mut font_system);

        let button = &mut button_manager.buttons_mut()[0];
        button.set_position(10.0, 10.0);

        let style = button.get_style();
        let width = style.width
            + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right;

        let height = style.height
            + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right;

        // Define test points: (x, y, should_hover)
        let test_points = [
            // Internal point (should hover)
            (10.0 + width as f64 / 2.0, 10.0 + height as f64 / 2.0, true),
            // Exact corners (should hover)
            (10.0, 10.0, true),                                // Top left
            (10.0 + width as f64, 10.0, true),                 // Top right
            (10.0, 10.0 + height as f64, true),                // Bottom left
            (10.0 + width as f64, 10.0 + height as f64, true), // Bottom right
            // Just outside corners (should not hover)
            (10.0 - 0.1, 10.0, false), // Top left
            (10.0, 10.0 - 0.1, false),
            (10.0 + width as f64 + 0.1, 10.0, false), // Top right
            (10.0 + width as f64, 10.0 - 0.1, false),
            (10.0 - 0.1, 10.0 + height as f64, false), // Bottom left
            (10.0, 10.0 + height as f64 + 0.1, false),
            (10.0 + width as f64 + 0.1, 10.0 + height as f64, false), // Bottom right
            (10.0 + width as f64, 10.0 + height as f64 + 0.1, false),
        ];

        test_points
            .iter()
            .enumerate()
            .for_each(|(i, (x, y, expected))| {
                assert_eq!(
                    button_manager.hover(*x, *y),
                    *expected,
                    "Test point {i} at ({x}, {y}) failed",
                );
            });
    }
}
