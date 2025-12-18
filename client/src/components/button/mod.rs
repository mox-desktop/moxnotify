mod action;
mod anchor;
mod dismiss;

use super::text::body;
use crate::{
    components::{self, Bounds, Component, Data},
    config::{
        self,
        button::ButtonState,
        keymaps::{self},
    },
    moxnotify::{common::Urgency, types::Action},
    rendering::text_renderer,
};
use action::ActionButton;
use anchor::AnchorButton;
use dismiss::DismissButton;
use glyphon::{FontSystem, TextArea};
use moxui::{shape_renderer, texture_renderer};
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
    context: components::Context,
    buttons: Vec<Box<dyn Button<Style = ButtonState>>>,
    urgency: Urgency,
    sender: Option<calloop::channel::Sender<crate::Event>>,
    _state: std::marker::PhantomData<State>,
}

impl ButtonManager<NotReady> {
    pub fn new(
        context: components::Context,
        urgency: Urgency,
        sender: Option<calloop::channel::Sender<crate::Event>>,
    ) -> Self {
        Self {
            context,
            buttons: Vec::new(),
            urgency,
            sender,
            _state: std::marker::PhantomData,
        }
    }

    pub fn add_actions(self, actions: &[Action], font_system: &mut FontSystem) -> Self {
        self.internal_add_actions(actions, font_system)
    }

    pub fn add_anchors(self, anchors: &[Arc<body::Anchor>], font_system: &mut FontSystem) -> Self {
        self.internal_add_anchors(anchors, font_system)
    }

    pub fn add_dismiss(mut self, font_system: &mut FontSystem) -> ButtonManager<Ready> {
        let font = &self
            .context
            .config
            .styles
            .default
            .buttons
            .dismiss
            .default
            .font;
        let text = text_renderer::Text::new(font, font_system, "X");

        let button = DismissButton {
            hint: Hint::new(self.context.clone(), "", font_system),
            text,
            x: 0.,
            y: 0.,
            state: State::Unhovered,
            tx: self.sender.clone(),
            context: self.context.clone(),
        };

        self.buttons.push(Box::new(button));

        ButtonManager {
            context: self.context,
            buttons: self.buttons,
            urgency: self.urgency,
            sender: self.sender,
            _state: std::marker::PhantomData,
        }
    }
}

impl ButtonManager<Ready> {
    pub fn add_actions(self, actions: &[Action], font_system: &mut FontSystem) -> Self {
        self.internal_add_actions(actions, font_system)
    }

    pub fn add_anchors(self, anchors: &[Arc<body::Anchor>], font_system: &mut FontSystem) -> Self {
        self.internal_add_anchors(anchors, font_system)
    }

    pub fn finish(mut self, font_system: &mut FontSystem) -> ButtonManager<Finished> {
        let hint_chars: Vec<char> = self
            .context
            .config
            .general
            .hint_characters
            .chars()
            .collect();
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
            let hint = Hint::new(self.context.clone(), &combination, font_system);

            button.set_hint(hint);
        });

        ButtonManager {
            buttons: self.buttons,
            urgency: self.urgency,
            sender: self.sender,
            context: self.context,
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

    pub fn instances(&self) -> Vec<shape_renderer::ShapeInstance> {
        let mut buttons = self
            .buttons
            .iter()
            .flat_map(|button| button.get_instances(self.urgency))
            .collect::<Vec<_>>();

        if self.context.ui_state.mode.load(Ordering::Relaxed) == keymaps::Mode::Hint
            && self.context.ui_state.selected_id.load(Ordering::Relaxed) == self.context.id
            && self.context.ui_state.selected.load(Ordering::Relaxed)
        {
            let hints = self
                .buttons
                .iter()
                .flat_map(|button| button.hint().get_instances(self.urgency))
                .collect::<Vec<_>>();
            buttons.extend_from_slice(&hints);
        }

        buttons
    }

    pub fn text_areas(&self) -> Vec<TextArea<'_>> {
        let mut text_areas = self
            .buttons
            .iter()
            .flat_map(|button| button.get_text_areas(self.urgency))
            .collect::<Vec<_>>();

        if self.context.ui_state.mode.load(Ordering::Relaxed) == keymaps::Mode::Hint
            && self.context.ui_state.selected_id.load(Ordering::Relaxed) == self.context.id
            && self.context.ui_state.selected.load(Ordering::Relaxed)
        {
            let hints = self
                .buttons
                .iter()
                .flat_map(|button| button.hint().get_text_areas(self.urgency));
            text_areas.extend(hints);
        }

        text_areas
    }

    pub fn get_data(&self) -> Vec<Data<'_>> {
        let mut data = self
            .buttons
            .iter()
            .flat_map(|button| button.get_data(self.urgency))
            .collect::<Vec<_>>();

        if self.context.ui_state.mode.load(Ordering::Relaxed) == keymaps::Mode::Hint
            && self.context.ui_state.selected_id.load(Ordering::Relaxed) == self.context.id
            && self.context.ui_state.selected.load(Ordering::Relaxed)
        {
            let hints = self
                .buttons
                .iter()
                .flat_map(|button| button.hint().get_data(self.urgency));
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

        let font = &self
            .context
            .config
            .styles
            .default
            .buttons
            .action
            .default
            .font;

        self.buttons.extend(anchors.iter().map(|anchor| {
            let text = text_renderer::Text::new(font, font_system, "");
            Box::new(AnchorButton {
                context: self.context.clone(),
                x: 0.,
                y: 0.,
                hint: Hint::new(self.context.clone(), "", font_system),
                state: State::Unhovered,
                tx: self.sender.clone(),
                text,
                anchor: Arc::clone(anchor),
            }) as Box<dyn Button<Style = ButtonState>>
        }));

        self
    }

    fn internal_add_actions(mut self, actions: &[Action], font_system: &mut FontSystem) -> Self {
        if actions.is_empty() {
            return self;
        }

        let mut buttons = actions
            .iter()
            .cloned()
            .map(|action| {
                let font = &self
                    .context
                    .config
                    .styles
                    .default
                    .buttons
                    .action
                    .default
                    .font;
                let text = text_renderer::Text::new(font, font_system, &action.label);

                Box::new(ActionButton {
                    context: self.context.clone(),
                    hint: Hint::new(self.context.clone(), "", font_system),
                    text,
                    x: 0.,
                    y: 0.,
                    action: action.key,
                    state: State::Unhovered,
                    width: 0.,
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
    combination: Box<str>,
    text: text_renderer::Text,
    context: components::Context,
    x: f32,
    y: f32,
}

impl Hint {
    pub fn new<T>(
        context: components::Context,
        combination: T,
        font_system: &mut FontSystem,
    ) -> Self
    where
        T: AsRef<str>,
    {
        Self {
            combination: combination.as_ref().into(),
            text: text_renderer::Text::new(
                &context.config.styles.default.font,
                font_system,
                combination.as_ref(),
            ),
            context,
            x: 0.,
            y: 0.,
        }
    }
}

impl Component for Hint {
    type Style = config::Hint;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        &self.context.config.styles.hover.hint
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

    fn get_instances(&self, urgency: Urgency) -> Vec<shape_renderer::ShapeInstance> {
        let style = &self.context.config.styles.hover.hint;
        let bounds = self.get_render_bounds();

        vec![shape_renderer::ShapeInstance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.color(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.color(urgency),
            scale: self.context.ui_state.scale.load(Ordering::Relaxed),
            depth: 0.7,
        }]
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn get_text_areas(&self, urgency: Urgency) -> Vec<TextArea<'_>> {
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
            scale: self.get_ui_state().scale.load(Ordering::Relaxed),
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
