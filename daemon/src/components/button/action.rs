use super::{Button, ButtonType, Hint, State};
use crate::{
    Urgency,
    components::{Bounds, Component},
    config::{Config, button::ButtonState},
    manager::UiState,
    rendering::{text_renderer, texture_renderer},
    utils::buffers,
};
use std::sync::{Arc, atomic::Ordering};

pub struct ActionButton {
    pub id: u32,
    pub app_name: Arc<str>,
    pub ui_state: UiState,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub config: Arc<Config>,
    pub text: text_renderer::Text,
    pub action: Arc<str>,
    pub state: State,
    pub width: f32,
    pub tx: Option<calloop::channel::Sender<crate::Event>>,
}

impl Component for ActionButton {
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

    fn get_ui_state(&self) -> &UiState {
        &self.ui_state
    }

    fn get_instances(&self, urgency: &Urgency) -> Vec<buffers::Instance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds();

        vec![buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [
                bounds.width - style.border.size.left - style.border.size.right,
                bounds.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: self.ui_state.scale.load(Ordering::Relaxed),
            depth: 0.8,
        }]
    }

    fn get_text_areas(&self, urgency: &Urgency) -> Vec<glyphon::TextArea<'_>> {
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
            scale: self.ui_state.scale.load(Ordering::Relaxed),
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

    fn get_style(&self) -> &Self::Style {
        let style = self.get_notification_style();

        match self.state() {
            State::Unhovered => &style.buttons.action.default,
            State::Hovered => &style.buttons.action.hover,
        }
    }

    fn get_bounds(&self) -> Bounds {
        let style = self.get_style();
        let text_extents = self.text.get_bounds();

        let width = style.width.resolve(self.width)
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

impl Button for ActionButton {
    fn hint(&self) -> &Hint {
        &self.hint
    }

    fn click(&self) {
        if let Some(tx) = self.tx.as_ref() {
            _ = tx.send(crate::Event::InvokeAction {
                id: self.id,
                key: Arc::clone(&self.action),
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
        self.state = State::Unhovered
    }

    fn set_hint(&mut self, hint: Hint) {
        self.hint = hint;
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Event,
        components::button::{Button, Hint, State},
        config::Config,
        manager::UiState,
        rendering::text_renderer::Text,
    };
    use glyphon::FontSystem;
    use std::sync::Arc;

    use super::ActionButton;

    #[test]
    fn test_action_button() {
        let config = Arc::new(Config::default());
        let ui_state = UiState::default();
        let hint = Hint::new(
            0,
            "",
            "".into(),
            Arc::clone(&config),
            &mut FontSystem::new(),
            ui_state.clone(),
        );

        let (tx, rx) = calloop::channel::channel();
        let test_id = 10;
        let test_action: Arc<str> = "test".into();
        let button = ActionButton {
            id: test_id,
            x: 0.,
            y: 0.,
            hint,
            text: Text::new(&config.styles.default.font, &mut FontSystem::new(), ""),
            state: State::Hovered,
            config: Arc::clone(&config),
            ui_state: ui_state.clone(),
            tx: Some(tx),
            width: 100.,
            action: Arc::clone(&test_action),
            app_name: "".into(),
        };

        button.click();

        let Event::InvokeAction { id, key } = rx.try_recv().unwrap() else {
            panic!("");
        };
        assert_eq!(id, test_id, "Button click should send button ID");
        assert_eq!(key, test_action, "Button click should send button ID");
    }

    #[test]
    fn test_multiple_action_buttons() {
        let config = Arc::new(Config::default());
        let ui_state = UiState::default();

        let (tx, text_rx1) = calloop::channel::channel();

        let test_id1 = 1;
        let test_action1: Arc<str> = "test1".into();
        let hint = Hint::new(
            0,
            "",
            "".into(),
            Arc::clone(&config),
            &mut FontSystem::new(),
            ui_state.clone(),
        );

        let button1 = ActionButton {
            id: test_id1,
            x: 0.,
            y: 0.,
            hint,
            text: Text::new(&config.styles.default.font, &mut FontSystem::new(), ""),
            state: State::Hovered,
            config: Arc::clone(&config),
            ui_state: ui_state.clone(),
            tx: Some(tx.clone()),
            width: 100.,
            action: Arc::clone(&test_action1),
            app_name: "".into(),
        };

        let (tx, text_rx2) = calloop::channel::channel();

        let test_id2 = 2;
        let test_action2: Arc<str> = "test2".into();
        let hint = Hint::new(
            0,
            "",
            "".into(),
            Arc::clone(&config),
            &mut FontSystem::new(),
            ui_state.clone(),
        );
        let button2 = ActionButton {
            id: test_id2,
            x: 0.,
            y: 0.,
            hint,
            text: Text::new(&config.styles.default.font, &mut FontSystem::new(), ""),
            state: State::Hovered,
            config: Arc::clone(&config),
            ui_state: ui_state.clone(),
            tx: Some(tx.clone()),
            width: 100.,
            action: Arc::clone(&test_action2),
            app_name: "".into(),
        };

        button1.click();
        let Event::InvokeAction { id, key } = text_rx1.try_recv().unwrap() else {
            panic!("");
        };
        assert_eq!(id, test_id1, "Button click should send button ID");
        assert_eq!(key, test_action1, "Button click should send button ID");

        assert!(text_rx2.try_recv().is_err());

        button2.click();
        let Event::InvokeAction { id, key } = text_rx2.try_recv().unwrap() else {
            panic!("");
        };
        assert_eq!(id, test_id2, "Button click should send button ID");
        assert_eq!(key, test_action2, "Button click should send button ID");

        assert!(text_rx1.try_recv().is_err());
    }
}
