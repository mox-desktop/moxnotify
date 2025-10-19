use super::{Button, Component, Hint, State};
use crate::{
    components::{self, Bounds, text::body::Anchor},
    config::button::ButtonState,
    rendering::text_renderer::Text,
};
use moxui::{shape_renderer, texture_renderer};
use std::sync::Arc;

pub struct AnchorButton {
    pub context: components::Context,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub text: Text,
    pub state: State,
    pub tx: Option<calloop::channel::Sender<crate::Event>>,
    pub anchor: Arc<Anchor>,
}

impl Component for AnchorButton {
    type Style = ButtonState;

    fn get_context(&self) -> &crate::components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        &self.context.config.styles.hover.buttons.dismiss.default
    }

    fn get_instances(&self, urgency: crate::Urgency) -> Vec<shape_renderer::ShapeInstance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds();
        vec![shape_renderer::ShapeInstance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.color(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.color(urgency),
            scale: 0.,
            depth: 0.8,
        }]
    }

    fn get_text_areas(&self, urgency: crate::Urgency) -> Vec<glyphon::TextArea<'_>> {
        let style = self.get_style();
        vec![glyphon::TextArea {
            buffer: &self.text.buffer,
            left: 0.,
            top: 0.,
            scale: 0.,
            bounds: glyphon::TextBounds {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            custom_glyphs: &[],
            default_color: style.font.color.into_glyphon(urgency),
        }]
    }

    fn get_bounds(&self) -> Bounds {
        let anchor_extents = self.anchor.get_bounds();

        Bounds {
            x: self.x + anchor_extents.x,
            y: self.y + anchor_extents.y,
            width: anchor_extents.width,
            height: anchor_extents.height,
        }
    }

    fn get_render_bounds(&self) -> Bounds {
        self.get_bounds()
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;

        let bounds = self.get_render_bounds();
        self.hint.set_position(bounds.x, bounds.y);
    }

    fn get_textures(&self) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }
}

impl Button for AnchorButton {
    fn hint(&self) -> &Hint {
        &self.hint
    }

    fn click(&self) {
        if let Some(tx) = self.tx.as_ref() {
            _ = tx.send(crate::Event::InvokeAnchor(Arc::clone(&self.anchor.href)));
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn button_type(&self) -> super::ButtonType {
        super::ButtonType::Anchor
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
