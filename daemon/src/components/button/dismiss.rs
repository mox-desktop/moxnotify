use super::{Button, ButtonType, Hint, State};
use crate::{
    Urgency,
    components::{self, Bounds, Component},
    config::button::ButtonState,
    rendering::text_renderer,
};
use moxui::{shape_renderer, texture_renderer};
use std::sync::atomic::Ordering;

pub struct DismissButton {
    pub context: components::Context,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub text: text_renderer::Text,
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
            rect_size: [
                bounds.width - style.border.size.left - style.border.size.right,
                bounds.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.color(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
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
        let (pl, _) = match (style.padding.left.is_auto(), style.padding.right.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.right.resolve(0.)),
            _ => (
                style.padding.left.resolve(0.),
                style.padding.right.resolve(0.),
            ),
        };

        let remaining_padding = extents.height - text_extents.height;
        let (pt, _) = match (style.padding.top.is_auto(), style.padding.bottom.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.bottom.resolve(0.)),
            _ => (
                style.padding.top.resolve(0.),
                style.padding.bottom.resolve(0.),
            ),
        };

        vec![glyphon::TextArea {
            buffer: &self.text.buffer,
            left: extents.x + style.border.size.left + style.padding.left.resolve(pl),
            top: extents.y + style.border.size.top + style.padding.top.resolve(pt),
            scale: self.get_ui_state().scale.load(Ordering::Relaxed),
            bounds: glyphon::TextBounds {
                left: (extents.x + style.border.size.left + style.padding.left.resolve(pl)) as i32,
                top: (extents.y + style.border.size.top + style.padding.top.resolve(pt)) as i32,
                right: (extents.x
                    + style.border.size.left
                    + style.padding.left.resolve(pl)
                    + text_extents.width) as i32,
                bottom: (extents.y
                    + style.border.size.top
                    + style.padding.top.resolve(pt)
                    + text_extents.height) as i32,
            },
            custom_glyphs: &[],
            default_color: style.font.color.into_glyphon(urgency),
        }]
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
            x: self.x,
            y: self.y,
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

#[cfg(test)]
mod tests {
    use super::DismissButton;
    use crate::{
        components::{
            self,
            button::{Button, Hint, State},
        },
        config::Config,
        manager::UiState,
        rendering::text_renderer::Text,
    };
    use glyphon::FontSystem;

    #[test]
    fn test_dismiss_button() {
        let test_id = 10;
        let context = components::Context {
            id: test_id,
            app_name: "".into(),
            config: Config::default().into(),
            ui_state: UiState::default(),
        };
        let hint = Hint::new(context.clone(), "", &mut FontSystem::new());

        let (tx, rx) = calloop::channel::channel();
        let button = DismissButton {
            x: 0.,
            y: 0.,
            hint,
            text: Text::new(
                &context.config.styles.default.font,
                &mut FontSystem::new(),
                "",
            ),
            state: State::Unhovered,
            tx: Some(tx),
            context,
        };

        button.click();

        if let crate::Event::Dismiss { all: false, id } = rx.try_recv().unwrap() {
            assert_eq!(id, test_id, "Button click should send button ID");
        };
    }
}
