use super::{Button, ButtonType, Hint, State};
use crate::components;
use crate::components::{Bounds, Component};
use crate::layout;
use crate::rendering::text::Text;
use config::client::Urgency;
use moxui::{shape_renderer, texture_renderer};
use std::sync::atomic::Ordering;

const DISMISS_BUTTON_WIDTH: f32 = layout::DISMISS_BUTTON_WIDTH;
const DISMISS_BUTTON_HEIGHT: f32 = layout::DISMISS_BUTTON_HEIGHT;

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
    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_instances(&self, _urgency: Urgency) -> Vec<shape_renderer::ShapeInstance> {
        let css = self.get_css_styles();
        let bounds = self.get_render_bounds();
        let is_hovered = matches!(self.state(), State::Hovered);

        let background = if is_hovered {
            css.button_dismiss
                .background
                .map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0])
                .unwrap_or([1.0, 1.0, 1.0, 1.0])
        } else {
            css.button_dismiss
                .background
                .map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0])
                .unwrap_or([0.0, 0.0, 0.0, 0.0])
        };

        let border_color = css
            .button_dismiss
            .border_color
            .map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0])
            .unwrap_or([0.0, 0.0, 0.0, 0.0]);

        vec![shape_renderer::ShapeInstance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: background,
            border_radius: layout::DISMISS_BUTTON_BORDER_RADIUS,
            border_size: [0.0; 4],
            border_color,
            scale: self.get_ui_state().scale.load(Ordering::Relaxed),
            depth: 0.8,
        }]
    }

    fn get_text_areas(&self, _urgency: Urgency) -> Vec<glyphon::TextArea<'_>> {
        let css = self.get_css_styles();
        let extents = self.get_render_bounds();
        let text_extents = self.text.get_bounds();

        let remaining_padding = extents.width - text_extents.width;
        let pl = remaining_padding / 2.;

        let remaining_padding = extents.height - text_extents.height;
        let pt = remaining_padding / 2.;

        let color = css
            .button_dismiss
            .color
            .map(|c| glyphon::Color::rgba(c[0], c[1], c[2], c[3]))
            .unwrap_or(glyphon::Color::rgba(255, 255, 255, 255));

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
            default_color: color,
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
