use super::{Button, Component, Hint, State};
use crate::{
    components::{text::body::Anchor, Bounds},
    config::{button::ButtonState, Config},
    manager::UiState,
    rendering::{text_renderer::Text, texture_renderer},
    utils::buffers,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

pub struct AnchorButton {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub config: Rc<Config>,
    pub text: Text,
    pub state: State,
    pub ui_state: Rc<RefCell<UiState>>,
    pub tx: Option<calloop::channel::Sender<Arc<str>>>,
    pub anchor: Rc<Anchor>,
    pub app_name: Arc<str>,
}

impl Component for AnchorButton {
    type Style = ButtonState;

    fn get_config(&self) -> &Config {
        &self.config
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_app_name(&self) -> &str {
        &self.app_name
    }

    fn get_ui_state(&self) -> std::cell::Ref<'_, UiState> {
        self.ui_state.borrow()
    }

    fn get_style(&self) -> &Self::Style {
        &self.config.styles.hover.buttons.dismiss.default
    }

    fn get_instances(&self, urgency: &crate::Urgency) -> Vec<buffers::Instance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds();
        vec![buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: 0.,
            depth: 0.8,
        }]
    }

    fn get_text_areas(&self, urgency: &crate::Urgency) -> Vec<glyphon::TextArea> {
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

    fn get_textures(&self) -> Vec<texture_renderer::TextureArea> {
        Vec::new()
    }
}

impl Button for AnchorButton {
    fn hint(&self) -> &Hint {
        &self.hint
    }

    fn click(&self) {
        if let Some(tx) = self.tx.as_ref() {
            _ = tx.send(Arc::clone(&self.anchor.href));
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
        self.state = State::Unhovered
    }

    fn set_hint(&mut self, hint: Hint) {
        self.hint = hint;
    }
}
