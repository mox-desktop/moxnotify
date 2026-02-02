use crate::components;
use crate::components::{Bounds, Component};
use crate::styles::{BorderRadius, Progress as ProgressStyle};
use config::client::Urgency;
use moxui::{shape_renderer, texture_renderer};
use std::sync::atomic::Ordering;

const PROGRESS_HEIGHT: f32 = 20.0;
const PROGRESS_MARGIN_TOP: f32 = 10.0;
const PROGRESS_BORDER_SIZE: f32 = 1.0;

pub struct Progress {
    context: components::Context,
    value: i32,
    x: f32,
    y: f32,
    width: f32,
}

impl Component for Progress {
    type Style = ProgressStyle;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        &self.get_notification_style().progress
    }

    fn get_bounds(&self) -> Bounds {
        let element_width = self.width;
        let remaining_space = self.width - element_width;

        let ml = remaining_space / 2.;

        let x_position = self.x + ml;

        Bounds {
            x: x_position,
            y: self.y,
            width: element_width,
            height: PROGRESS_HEIGHT + PROGRESS_MARGIN_TOP,
        }
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn get_render_bounds(&self) -> Bounds {
        let bounds = self.get_bounds();

        let remaining_space = self.width - bounds.width;
        let ml = remaining_space / 2.;

        Bounds {
            x: bounds.x + ml,
            y: bounds.y + PROGRESS_MARGIN_TOP,
            width: bounds.width - ml,
            height: bounds.height - PROGRESS_MARGIN_TOP,
        }
    }

    fn get_text_areas(&self, _: Urgency) -> Vec<glyphon::TextArea<'_>> {
        vec![]
    }

    fn get_instances(&self, urgency: Urgency) -> Vec<shape_renderer::ShapeInstance> {
        let extents = self.get_render_bounds();

        let progress_ratio = (self.value as f32 / 100.0).min(1.0);

        let mut instances = Vec::new();
        let complete_width = (extents.width * progress_ratio).max(0.);

        let style = self.get_style();

        if complete_width > 0.0 {
            let border_size = if self.value < 100 {
                [PROGRESS_BORDER_SIZE, 0.0, PROGRESS_BORDER_SIZE, PROGRESS_BORDER_SIZE]
            } else {
                [PROGRESS_BORDER_SIZE; 4]
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
                border_size,
                border_color: style.border.color.color(urgency),
                scale: self.get_ui_state().scale.load(Ordering::Relaxed),
                depth: 0.8,
            });
        }

        if self.value < 100 {
            let incomplete_width = extents.width - complete_width;

            if incomplete_width > 0.0 {
                let border_size = if self.value > 0 {
                    [0.0, PROGRESS_BORDER_SIZE, PROGRESS_BORDER_SIZE, PROGRESS_BORDER_SIZE]
                } else {
                    [PROGRESS_BORDER_SIZE; 4]
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
                    border_size,
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
