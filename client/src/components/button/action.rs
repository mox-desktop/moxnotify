use super::{Button, ButtonType, Hint, State};
use crate::components;
use crate::components::{Bounds, Component};
use crate::rendering::text::Text;
use crate::styles::ButtonState;
use config::client::Urgency;
use moxui::{shape_renderer, texture_renderer};
use std::sync::atomic::Ordering;

// Hardcoded layout constants (previously configurable)
const ACTION_BUTTON_PADDING_TOP: f32 = 5.0;
const ACTION_BUTTON_PADDING_BOTTOM: f32 = 5.0;
const ACTION_BUTTON_MARGIN_LEFT: f32 = 5.0;
const ACTION_BUTTON_MARGIN_RIGHT: f32 = 5.0;
const ACTION_BUTTON_BORDER_SIZE: f32 = 1.0;

pub struct ActionButton {
    pub context: components::Context,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub text: Text,
    pub action: String,
    pub state: State,
    pub width: f32,
    pub tx: Option<calloop::channel::Sender<crate::Event>>,
    pub uuid: String,
}

impl Component for ActionButton {
    type Style = ButtonState;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_instances(&self, urgency: Urgency) -> Vec<shape_renderer::ShapeInstance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds();

        vec![shape_renderer::ShapeInstance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [
                bounds.width - ACTION_BUTTON_BORDER_SIZE * 2.0,
                bounds.height - ACTION_BUTTON_BORDER_SIZE * 2.0,
            ],
            rect_color: style.background.color(urgency),
            border_radius: style.border.radius.into(),
            border_size: [ACTION_BUTTON_BORDER_SIZE; 4],
            border_color: style.border.color.color(urgency),
            scale: self.get_ui_state().scale.load(Ordering::Relaxed),
            depth: 0.8,
        }]
    }

    fn get_text_areas(&self, urgency: Urgency) -> Vec<glyphon::TextArea<'_>> {
        let extents = self.get_render_bounds();
        let style = self.get_style();
        let text_extents = self.text.get_bounds();

        let remaining_padding = extents.width - text_extents.width;
        let pl = remaining_padding / 2.;
        let remaining_padding = extents.height - text_extents.height;
        let pt = remaining_padding / 2.;

        vec![glyphon::TextArea {
            buffer: &self.text.buffer,
            left: extents.x + ACTION_BUTTON_BORDER_SIZE + pl,
            top: extents.y + ACTION_BUTTON_BORDER_SIZE + pt,
            scale: self.get_ui_state().scale.load(Ordering::Relaxed),
            bounds: glyphon::TextBounds {
                left: (extents.x + ACTION_BUTTON_BORDER_SIZE + pl) as i32,
                top: (extents.y + ACTION_BUTTON_BORDER_SIZE + pt) as i32,
                right: (extents.x + ACTION_BUTTON_BORDER_SIZE + pl + text_extents.width) as i32,
                bottom: (extents.y + ACTION_BUTTON_BORDER_SIZE + pt + text_extents.height) as i32,
            },
            custom_glyphs: &[],
            default_color: style.font.color.into_glyphon(urgency),
        }]
    }

    fn get_style(&self) -> &Self::Style {
        let style = self.get_notification_style();

        match self.state() {
            State::Unhovered => &style.buttons.action.default,
            State::Hovered => &style.buttons.action.hover,
        }
    }

    fn get_bounds(&self) -> Bounds {
        let text_extents = self.text.get_bounds();

        let width = self.width
            + ACTION_BUTTON_BORDER_SIZE * 2.0
            + ACTION_BUTTON_MARGIN_LEFT
            + ACTION_BUTTON_MARGIN_RIGHT;

        let height = text_extents.height
            + ACTION_BUTTON_BORDER_SIZE * 2.0
            + ACTION_BUTTON_PADDING_TOP
            + ACTION_BUTTON_PADDING_BOTTOM;

        Bounds {
            x: self.x,
            y: self.y,
            width,
            height,
        }
    }

    fn get_render_bounds(&self) -> Bounds {
        let bounds = self.get_bounds();

        Bounds {
            x: bounds.x + ACTION_BUTTON_MARGIN_LEFT,
            y: bounds.y,
            width: bounds.width - ACTION_BUTTON_MARGIN_LEFT - ACTION_BUTTON_MARGIN_RIGHT,
            height: bounds.height,
        }
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        self.text.set_buffer_position(x, y);

        let bounds = self.get_render_bounds();
        self.hint.set_position(bounds.x, bounds.y);
    }

    fn get_textures(&self) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }
}

impl Button for ActionButton {
    fn hint(&self) -> &Hint {
        &self.hint
    }

    fn click(&self) {
        if let Some(tx) = self.tx.as_ref() {
            _ = tx.send(crate::Event::InvokeAction {
                id: self.get_id(),
                key: self.action.clone(),
                uuid: self.uuid.clone(),
            });
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn button_type(&self) -> ButtonType {
        ButtonType::Action
    }

    fn state(&self) -> State {
        self.state
    }

    fn hover(&mut self) {
        self.state = State::Hovered;
    }

    fn unhover(&mut self) {
        self.state = State::Unhovered;
    }

    fn set_hint(&mut self, hint: Hint) {
        self.hint = hint;
    }
}
