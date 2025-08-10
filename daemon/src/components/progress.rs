use crate::{
    Urgency,
    components::{self, Component},
    config::{self, Insets, Size, border::BorderRadius},
    utils::taffy::{GlobalLayout, NodeContext},
};
use moxui::{shape_renderer, texture_renderer};
use std::sync::atomic::Ordering;
use taffy::style_helpers::{auto, length, line, span};

pub struct Progress {
    node: taffy::NodeId,
    context: components::Context,
    value: i32,
    x: f32,
    y: f32,
    width: f32,
}

impl Component for Progress {
    type Style = config::Progress;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        &self.get_notification_style().progress
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<NodeContext>) {
        let style = self.get_style();
        self.node = tree
            .new_leaf(taffy::Style {
                grid_row: line(4),
                grid_column: span(3),
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
    }

    fn apply_computed_layout(&mut self, tree: &taffy::TaffyTree<NodeContext>) {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        self.x = layout.location.x;
        self.y = layout.location.y;
    }

    fn get_text_areas(
        &self,
        _: &taffy::TaffyTree<NodeContext>,
        _: Urgency,
    ) -> Vec<glyphon::TextArea<'_>> {
        Vec::new()
    }

    fn get_instances(
        &self,
        tree: &taffy::TaffyTree<NodeContext>,
        urgency: Urgency,
    ) -> Vec<shape_renderer::ShapeInstance> {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        let progress_ratio = (self.value as f32 / 100.0).min(1.0);

        let mut instances = Vec::new();
        let complete_width = (layout.content_box_width() * progress_ratio).max(0.);

        let style = self.get_style();

        if complete_width > 0.0 {
            let border_size = if self.value < 100 {
                Insets {
                    right: Size::Value(0.),
                    ..style.border.size
                }
            } else {
                style.border.size
            };

            let border_radius = if self.value < 100 {
                BorderRadius {
                    top_right: 0.0,
                    bottom_right: 0.0,
                    ..style.border.radius
                }
            } else {
                style.border.radius
            };

            instances.push(shape_renderer::ShapeInstance {
                rect_pos: [layout.location.x, layout.location.y],
                rect_size: [complete_width, layout.content_box_height()],
                rect_color: style.complete_color.color(urgency),
                border_radius: border_radius.into(),
                border_size: border_size.into(),
                border_color: style.border.color.color(urgency),
                scale: self.get_ui_state().scale.load(Ordering::Relaxed),
                depth: 0.8,
            });
        }

        if self.value < 100 {
            let incomplete_width = layout.content_box_width() - complete_width;

            if incomplete_width > 0.0 {
                let border_size = if self.value > 0 {
                    Insets {
                        left: Size::Value(0.),
                        ..style.border.size
                    }
                } else {
                    style.border.size
                };

                let border_radius = if self.value > 0 {
                    BorderRadius {
                        top_left: 0.0,
                        bottom_left: 0.0,
                        ..style.border.radius
                    }
                } else {
                    style.border.radius
                };

                instances.push(shape_renderer::ShapeInstance {
                    rect_pos: [layout.location.x + complete_width, layout.location.y],
                    rect_size: [incomplete_width, layout.content_box_height()],
                    rect_color: style.incomplete_color.color(urgency),
                    border_radius: border_radius.into(),
                    border_size: border_size.into(),
                    border_color: style.border.color.color(urgency),
                    scale: self.get_ui_state().scale.load(Ordering::Relaxed),
                    depth: 0.8,
                });
            }
        }

        instances
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

impl Progress {
    #[must_use]
    pub fn new(
        tree: &mut taffy::TaffyTree<NodeContext>,
        context: components::Context,
        value: i32,
    ) -> Self {
        let node = tree.new_leaf(taffy::Style::DEFAULT).unwrap();

        Self {
            context,
            value,
            x: 0.,
            y: 0.,
            width: 0.,
            node,
        }
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width;
    }

    pub fn set_value(&mut self, value: i32) {
        self.value = value;
    }
}
