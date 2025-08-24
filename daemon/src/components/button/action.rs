use super::{Button, ButtonType, Hint, State};
use crate::{
    Urgency,
    components::{self, Component},
    config::button::ButtonState,
    rendering::{text_renderer, texture_renderer},
    utils::{buffers, taffy::GlobalLayout},
};
use std::sync::{Arc, atomic::Ordering};
use taffy::style_helpers::{auto, length, line};

pub struct ActionButton {
    pub node: taffy::NodeId,
    pub context: components::Context,
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub text: text_renderer::Text,
    pub action: Arc<str>,
    pub state: State,
    pub width: f32,
    pub tx: Option<calloop::channel::Sender<crate::Event>>,
}

impl Component for ActionButton {
    type Style = ButtonState;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_instances(
        &self,
        tree: &taffy::TaffyTree<()>,
        urgency: Urgency,
    ) -> Vec<buffers::Instance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds(tree);

        vec![buffers::Instance {
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

    fn get_text_areas(
        &self,
        tree: &taffy::TaffyTree<()>,
        urgency: Urgency,
    ) -> Vec<glyphon::TextArea<'_>> {
        let extents = self.get_render_bounds(tree);
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

    fn get_style(&self) -> &Self::Style {
        let style = self.get_notification_style();

        match self.state() {
            State::Unhovered => &style.buttons.action.default,
            State::Hovered => &style.buttons.action.hover,
        }
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        let style = self.get_style();
        self.node = tree
            .new_leaf(taffy::Style {
                grid_column: line(self.index as i16 + 1),
                flex_grow: 1.0,
                flex_basis: auto(),
                min_size: taffy::Size {
                    width: length(self.text.get_bounds().width),
                    height: length(self.text.get_bounds().height),
                },
                size: taffy::Size {
                    width: if style.width.is_auto() {
                        auto()
                    } else {
                        length(style.width.resolve(0.))
                    },
                    height: if style.height.is_auto() {
                        auto()
                    } else {
                        length(style.height.resolve(0.))
                    },
                },
                padding: taffy::Rect {
                    left: length(style.padding.left.resolve(0.)),
                    right: length(style.padding.right.resolve(0.)),
                    top: length(style.padding.top.resolve(0.)),
                    bottom: length(style.padding.bottom.resolve(0.)),
                },
                margin: taffy::Rect {
                    left: if style.margin.left.is_auto() {
                        auto()
                    } else {
                        length(style.margin.left.resolve(0.))
                    },
                    right: if style.margin.right.is_auto() {
                        auto()
                    } else {
                        length(style.margin.right.resolve(0.))
                    },
                    top: if style.margin.top.is_auto() {
                        auto()
                    } else {
                        length(style.margin.top.resolve(0.))
                    },
                    bottom: if style.margin.bottom.is_auto() {
                        auto()
                    } else {
                        length(style.margin.bottom.resolve(0.))
                    },
                },
                border: taffy::Rect {
                    left: length(style.border.size.left.resolve(0.)),
                    right: length(style.border.size.left.resolve(0.)),
                    top: length(style.border.size.left.resolve(0.)),
                    bottom: length(style.border.size.left.resolve(0.)),
                },
                ..Default::default()
            })
            .unwrap();

        self.hint.update_layout(tree);
    }

    fn apply_computed_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        self.x = layout.location.x;
        self.y = layout.location.y;
        self.width = layout.content_box_width();
        self.text.set_buffer_position(self.x, self.y);
        self.hint.apply_computed_layout(tree);
    }

    fn get_textures(&self, tree: &taffy::TaffyTree<()>) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }

    fn get_node_id(&self) -> taffy::NodeId {
        self.node
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
        self.state = State::Unhovered;
    }

    fn set_hint(&mut self, hint: Hint) {
        self.hint = hint;
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Event,
        components::{
            self,
            button::{Button, Hint, State},
        },
        config::Config,
        manager::UiState,
        rendering::text_renderer::Text,
    };
    use glyphon::FontSystem;
    use std::sync::Arc;

    use super::ActionButton;

    #[test]
    fn test_action_button() {
        let test_id = 10;
        let context = components::Context {
            id: test_id,
            app_name: "".into(),
            config: Config::default().into(),
            ui_state: UiState::default(),
        };
        let hint = Hint::new(context.clone(), "", &mut FontSystem::new());

        let (tx, rx) = calloop::channel::channel();
        let test_action: Arc<str> = "test".into();
        let button = ActionButton {
            x: 0.,
            y: 0.,
            hint,
            text: Text::new(
                &context.config.styles.default.font,
                &mut FontSystem::new(),
                "",
            ),
            state: State::Hovered,
            tx: Some(tx),
            width: 100.,
            action: Arc::clone(&test_action),
            context,
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
        let (tx, text_rx1) = calloop::channel::channel();

        let test_id1 = 1;
        let test_action1: Arc<str> = "test1".into();
        let context = components::Context {
            id: test_id1,
            app_name: Arc::clone(&test_action1),
            config: Config::default().into(),
            ui_state: UiState::default(),
        };
        let hint = Hint::new(context.clone(), "", &mut FontSystem::new());

        let button1 = ActionButton {
            x: 0.,
            y: 0.,
            hint,
            text: Text::new(
                &context.config.styles.default.font,
                &mut FontSystem::new(),
                "",
            ),
            state: State::Hovered,
            tx: Some(tx.clone()),
            width: 100.,
            action: Arc::clone(&test_action1),
            context,
        };

        let (tx, text_rx2) = calloop::channel::channel();

        let test_id2 = 2;
        let test_action2: Arc<str> = "test2".into();
        let context = components::Context {
            id: test_id2,
            app_name: Arc::clone(&test_action2),
            config: Config::default().into(),
            ui_state: UiState::default(),
        };
        let hint = Hint::new(context.clone(), "", &mut FontSystem::new());
        let button2 = ActionButton {
            x: 0.,
            y: 0.,
            hint,
            text: Text::new(
                &context.config.styles.default.font,
                &mut FontSystem::new(),
                "",
            ),
            state: State::Hovered,
            tx: Some(tx.clone()),
            width: 100.,
            action: Arc::clone(&test_action2),
            context,
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
