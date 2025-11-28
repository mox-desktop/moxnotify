use super::{Button, ButtonType, Hint, State};
use crate::{
    Urgency,
    components::{self, Component},
    config::button::ButtonState,
    rendering::text_renderer,
    utils::taffy::{GlobalLayout, NodeContext},
};
use moxui::{shape_renderer, texture_renderer};
use std::sync::{Arc, atomic::Ordering};
use taffy::style_helpers::{auto, length};

pub struct ActionButton {
    pub node: taffy::NodeId,
    pub context: components::Context,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub text: text_renderer::TextContext,
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
        tree: &taffy::TaffyTree<NodeContext>,
        urgency: Urgency,
    ) -> Vec<shape_renderer::ShapeInstance> {
        let style = self.get_style();
        let layout = tree.global_layout(self.get_node_id()).unwrap();

        vec![shape_renderer::ShapeInstance {
            rect_pos: [layout.location.x, layout.location.y],
            rect_size: [layout.size.width, layout.size.height],
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
        let layout = tree.global_layout(self.get_node_id()).unwrap();

        let remaining_width = layout.content_box_width() - text_extents.width;
        let (pl, _) = match (style.padding.left.is_auto(), style.padding.right.is_auto()) {
            (true, true) => (remaining_width / 2., remaining_width / 2.),
            (true, false) => (remaining_width, style.padding.right.resolve(0.)),
            _ => (
                style.padding.left.resolve(0.),
                style.padding.right.resolve(0.),
            ),
        };

        let remaining_height = layout.content_box_height() - text_extents.height;
        let (pt, _) = match (style.padding.top.is_auto(), style.padding.bottom.is_auto()) {
            (true, true) => (remaining_height / 2., remaining_height / 2.),
            (true, false) => (remaining_height, style.padding.bottom.resolve(0.)),
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

    fn get_style(&self) -> &Self::Style {
        let style = self.get_notification_style();

        match self.state() {
            State::Unhovered => &style.buttons.action.default,
            State::Hovered => &style.buttons.action.hover,
        }
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<NodeContext>) {
        let style = self.get_style();
        let text_bounds = self.text.get_bounds();
        let padding_x = style.padding.left.resolve(0.) + style.padding.right.resolve(0.);
        let padding_y = style.padding.top.resolve(0.) + style.padding.bottom.resolve(0.);
        let border_x = style.border.size.left.resolve(0.) + style.border.size.right.resolve(0.);
        let border_y = style.border.size.top.resolve(0.) + style.border.size.bottom.resolve(0.);
        let intrinsic_width = text_bounds.width + padding_x + border_x;
        let intrinsic_height = text_bounds.height + padding_y + border_y;
        let resolved_width = if style.width.is_auto() {
            intrinsic_width
        } else {
            style.width.resolve(0.)
        };
        let resolved_height = if style.height.is_auto() {
            intrinsic_height
        } else {
            style.height.resolve(0.)
        };

        self.node = tree
            .new_leaf(taffy::Style {
                flex_grow: 1.0,
                flex_shrink: 0.0,
                text_align: taffy::TextAlign::LegacyCenter,
                min_size: taffy::Size {
                    width: length(intrinsic_width),
                    height: length(intrinsic_height),
                },
                size: taffy::Size {
                    width: if style.width.is_auto() {
                        auto()
                    } else {
                        length(resolved_width)
                    },
                    height: length(resolved_height),
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

    fn apply_computed_layout(&mut self, tree: &taffy::TaffyTree<NodeContext>) {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        let style = self.get_style();
        let text_extents = self.text.get_bounds();
        let padding_left = style.padding.left;
        let padding_right = style.padding.right;
        let padding_top = style.padding.top;
        let padding_bottom = style.padding.bottom;
        let border_left = style.border.size.left;
        let border_top = style.border.size.top;
        let remaining_width = layout.content_box_width() - text_extents.width;
        let pl = match (padding_left.is_auto(), padding_right.is_auto()) {
            (true, true) => remaining_width / 2.,
            (true, false) => remaining_width,
            _ => padding_left.resolve(0.),
        };
        let remaining_height = layout.content_box_height() - text_extents.height;
        let pt = match (padding_top.is_auto(), padding_bottom.is_auto()) {
            (true, true) => remaining_height / 2.,
            (true, false) => remaining_height,
            _ => padding_top.resolve(0.),
        };
        self.x = layout.location.x;
        self.y = layout.location.y;
        self.width = layout.size.width;
        self.text.set_buffer_position(
            self.x + border_left + padding_left.resolve(pl),
            self.y + border_top + padding_top.resolve(pt),
        );
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
