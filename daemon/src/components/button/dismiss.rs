use super::{Button, ButtonType, Hint, State};
use crate::{
    Urgency,
    components::{self, Component},
    config::button::ButtonState,
    rendering::text_renderer,
    utils::taffy::{GlobalLayout, NodeContext},
};
use moxui::{shape_renderer, texture_renderer};
use std::sync::atomic::Ordering;
use taffy::style_helpers::{auto, length, line};

pub struct DismissButton {
    pub node: taffy::NodeId,
    pub context: components::Context,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub text: text_renderer::TextContext,
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

    fn get_instances(
        &self,
        tree: &taffy::TaffyTree<NodeContext>,
        urgency: Urgency,
    ) -> Vec<shape_renderer::ShapeInstance> {
        let style = self.get_style();
        let layout = tree.global_layout(self.node).unwrap();
        vec![shape_renderer::ShapeInstance {
            rect_pos: [layout.location.x, layout.location.y],
            rect_size: [layout.content_box_width(), layout.content_box_height()],
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
        tree: &taffy::TaffyTree<NodeContext>,
        urgency: Urgency,
    ) -> Vec<glyphon::TextArea<'_>> {
        let style = self.get_style();
        let text_extents = self.text.get_bounds();
        let layout = tree.global_layout(self.node).unwrap();

        let remaining_padding = layout.content_box_width() - text_extents.width;
        let (pl, _) = match (style.padding.left.is_auto(), style.padding.right.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.right.resolve(0.)),
            _ => (
                style.padding.left.resolve(0.),
                style.padding.right.resolve(0.),
            ),
        };

        let remaining_padding = layout.content_box_height() - text_extents.height;
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
            left: layout.location.x + style.border.size.left + style.padding.left.resolve(pl),
            top: layout.location.y + style.border.size.top + style.padding.top.resolve(pt),
            scale: self.get_ui_state().scale.load(Ordering::Relaxed),
            bounds: glyphon::TextBounds {
                left: (layout.location.x + style.border.size.left + style.padding.left.resolve(pl))
                    as i32,
                top: (layout.location.y + style.border.size.top + style.padding.top.resolve(pt))
                    as i32,
                right: (layout.location.x
                    + style.border.size.left
                    + style.padding.left.resolve(pl)
                    + text_extents.width) as i32,
                bottom: (layout.location.y
                    + style.border.size.top
                    + style.padding.top.resolve(pt)
                    + text_extents.height) as i32,
            },
            custom_glyphs: &[],
            default_color: style.font.color.into_glyphon(urgency),
        }]
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<NodeContext>) {
        let style = self.get_style();
        self.node = tree
            .new_leaf(taffy::Style {
                grid_row: line(1),
                grid_column: line(3),
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
                    bottom: if style.margin.bottom.is_auto() {
                        auto()
                    } else {
                        length(style.margin.bottom.resolve(0.))
                    },
                    top: if style.margin.top.is_auto() {
                        auto()
                    } else {
                        length(style.margin.top.resolve(0.))
                    },
                },
                padding: taffy::Rect {
                    left: length(style.padding.left.resolve(0.)),
                    right: length(style.padding.right.resolve(0.)),
                    top: length(style.padding.top.resolve(0.)),
                    bottom: length(style.padding.bottom.resolve(0.)),
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

    fn apply_computed_layout(&mut self, tree: &taffy::TaffyTree<NodeContext>) {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        self.x = layout.location.x;
        self.y = layout.location.y;
        self.text.set_buffer_position(self.x, self.y);
        self.hint.apply_computed_layout(tree);
    }

    fn get_textures(
        &self,
        _: &taffy::TaffyTree<NodeContext>,
    ) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }

    fn get_node_id(&self) -> taffy::NodeId {
        self.node
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
        rendering::text_renderer::TextRenderer,
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
