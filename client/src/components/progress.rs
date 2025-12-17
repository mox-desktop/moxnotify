use crate::{
    components::{self, Bounds, Component},
    config::{self, Insets, Size, border::BorderRadius},
};
use moxui::{shape_renderer, texture_renderer};
use std::sync::atomic::Ordering;

pub struct Progress {
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

    fn get_bounds(&self) -> Bounds {
        let style = self.get_config().find_style(
            self.get_app_name(),
            self.get_ui_state().selected_id.load(Ordering::Relaxed) == self.get_id()
                && self.get_ui_state().selected.load(Ordering::Relaxed),
        );

        let element_width = style.progress.width.resolve(self.width);
        let remaining_space = self.width - element_width;

        let (resolved_ml, _) = match (
            style.progress.margin.left.is_auto(),
            style.progress.margin.right.is_auto(),
        ) {
            (true, true) => {
                let margin = remaining_space / 2.0;
                (margin, margin)
            }
            (true, false) => {
                let mr = style.progress.margin.right.resolve(0.);
                (remaining_space, mr)
            }
            _ => (
                style.progress.margin.left.resolve(0.),
                style.progress.margin.right.resolve(0.),
            ),
        };

        let x_position = self.x + resolved_ml;

        Bounds {
            x: x_position,
            y: self.y,
            width: element_width,
            height: style.progress.height
                + style.progress.margin.top
                + style.progress.margin.bottom,
        }
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn get_render_bounds(&self) -> Bounds {
        let bounds = self.get_bounds();

        let style = self.get_config().find_style(
            self.get_app_name(),
            self.get_ui_state().selected_id.load(Ordering::Relaxed) == self.get_id()
                && self.get_ui_state().selected.load(Ordering::Relaxed),
        );

        let remaining_space = self.width - bounds.width;
        let (margin_left, _) = match (
            style.progress.margin.left.is_auto(),
            style.progress.margin.right.is_auto(),
        ) {
            (true, true) => {
                let margin = remaining_space / 2.0;
                (margin, margin)
            }
            (true, false) => {
                let mr = style.progress.margin.right.resolve(0.);
                (remaining_space, mr)
            }
            _ => (
                style.progress.margin.left.resolve(0.),
                style.progress.margin.right.resolve(0.),
            ),
        };

        Bounds {
            x: bounds.x + margin_left,
            y: bounds.y + style.progress.margin.top,
            width: bounds.width - margin_left - style.progress.margin.right,
            height: bounds.height - style.progress.margin.top - style.progress.margin.bottom,
        }
    }

    fn get_text_areas(&self, _: i32) -> Vec<glyphon::TextArea<'_>> {
        vec![]
    }

    fn get_instances(&self, urgency: i32) -> Vec<shape_renderer::ShapeInstance> {
        let extents = self.get_render_bounds();

        let progress_ratio = (self.value as f32 / 100.0).min(1.0);

        let mut instances = Vec::new();
        let complete_width = (extents.width * progress_ratio).max(0.);

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
                rect_pos: [extents.x, extents.y],
                rect_size: [complete_width, extents.height],
                rect_color: style.complete_color.color(urgency),
                border_radius: border_radius.into(),
                border_size: border_size.into(),
                border_color: style.border.color.color(urgency),
                scale: self.get_ui_state().scale.load(Ordering::Relaxed),
                depth: 0.8,
            });
        }

        if self.value < 100 {
            let incomplete_width = extents.width - complete_width;

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
                    rect_pos: [extents.x + complete_width, extents.y],
                    rect_size: [incomplete_width, extents.height],
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

    fn get_textures(&self) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }
}

impl Progress {
    #[must_use]
    pub fn new(context: components::Context, value: i32) -> Self {
        Self {
            context,
            value,
            x: 0.,
            y: 0.,
            width: 0.,
        }
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width;
    }

    pub fn set_value(&mut self, value: i32) {
        self.value = value;
    }
}
