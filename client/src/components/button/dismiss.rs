use super::{Button, ButtonType, Hint, State};
use crate::components;
use crate::components::{Bounds, Component};
use crate::rendering::text::Text;
use config::client::Urgency;
use config::client::button::ButtonState;
use moxui::{shape_renderer, texture_renderer};
use std::sync::atomic::Ordering;

// Hardcoded layout constants (previously configurable)
const DISMISS_BUTTON_WIDTH: f32 = 20.0;
const DISMISS_BUTTON_HEIGHT: f32 = 20.0;

pub struct DismissButton {
    pub context: components::Context,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub text: Text,
    pub state: State,
    pub tx: Option<calloop::channel::Sender<crate::Event>>,
}

impl Component for DismissButton {
    type Style = ButtonState;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        let style = self.get_notification_style();
        match self.state() {
            State::Unhovered => &style.buttons.dismiss.default,
            State::Hovered => &style.buttons.dismiss.hover,
        }
    }

    fn get_instances(&self, urgency: Urgency) -> Vec<shape_renderer::ShapeInstance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds();

        vec![shape_renderer::ShapeInstance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.color(urgency),
            border_radius: style.border.radius.into(),
            border_size: [0.0; 4],
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
            left: extents.x + pl,
            top: extents.y + pt,
            scale: self.get_ui_state().scale.load(Ordering::Relaxed),
            bounds: glyphon::TextBounds {
                left: (extents.x + pl) as i32,
                top: (extents.y + pt) as i32,
                right: (extents.x + pl + text_extents.width) as i32,
                bottom: (extents.y + pt + text_extents.height) as i32,
            },
            custom_glyphs: &[],
            default_color: style.font.color.into_glyphon(urgency),
        }]
    }

    fn get_bounds(&self) -> Bounds {
        let _text_extents = self.text.get_bounds();

        let width = DISMISS_BUTTON_WIDTH;
        let height = DISMISS_BUTTON_HEIGHT;

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
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
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

impl Button for DismissButton {
    fn hint(&self) -> &Hint {
        &self.hint
    }

    fn click(&self) {
        if let Some(tx) = self.tx.as_ref() {
            _ = tx.send(crate::Event::Dismiss {
                all: false,
                id: self.get_id(),
            });
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn button_type(&self) -> ButtonType {
        ButtonType::Dismiss
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
