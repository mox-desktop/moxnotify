use config::client::Urgency;
use crate::components;
use crate::components::{Bounds, Component};
use crate::layout;
use moxui::{shape_renderer, texture_renderer};
use std::sync::atomic::Ordering;

const PROGRESS_HEIGHT: f32 = layout::PROGRESS_HEIGHT;
const PROGRESS_MARGIN_TOP: f32 = layout::PROGRESS_MARGIN_TOP;
const PROGRESS_BORDER_SIZE: f32 = layout::PROGRESS_BORDER_SIZE;

pub struct Progress {
    context: components::Context,
    value: i32,
    x: f32,
    y: f32,
    width: f32,
}

impl Component for Progress {
    fn get_context(&self) -> &components::Context {
        &self.context
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
        let css = self.get_css_styles();

        let progress_ratio = (self.value as f32 / 100.0).min(1.0);

        let mut instances = Vec::new();
        let complete_width = (extents.width * progress_ratio).max(0.);

        // Get colors from CSS with defaults
        let complete_color = css
            .progress_complete
            .background
            .or(css.progress_complete.color)
            .map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0])
            .unwrap_or(match urgency {
                Urgency::Low => [0.949, 0.804, 0.804, 1.0],      // #f2cdcd
                Urgency::Normal => [0.949, 0.804, 0.804, 1.0],   // #f2cdcd
                Urgency::Critical => [0.953, 0.545, 0.659, 1.0], // #f38ba8
            });

        let incomplete_color = css
            .progress
            .background
            .map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0])
            .unwrap_or([0.0, 0.0, 0.0, 0.0]);

        let border_color = css
            .progress
            .border_color
            .or(css.progress_complete.border_color)
            .map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0])
            .unwrap_or(match urgency {
                Urgency::Low => [0.651, 0.890, 0.631, 1.0],      // #a6e3a1
                Urgency::Normal => [0.796, 0.651, 0.969, 1.0],   // #cba6f7
                Urgency::Critical => [0.953, 0.545, 0.659, 1.0], // #f38ba8
            });

        // PROGRESS_BORDER_RADIUS is [bottom_right, top_right, bottom_left, top_left]
        let base_radius = layout::PROGRESS_BORDER_RADIUS;

        if complete_width > 0.0 {
            let border_size = if self.value < 100 {
                [PROGRESS_BORDER_SIZE, 0.0, PROGRESS_BORDER_SIZE, PROGRESS_BORDER_SIZE]
            } else {
                [PROGRESS_BORDER_SIZE; 4]
            };

            let border_radius = if self.value < 100 {
                // Zero out right corners when incomplete
                [base_radius[0], 0.0, base_radius[2], base_radius[3]]
            } else {
                base_radius
            };

            instances.push(shape_renderer::ShapeInstance {
                rect_pos: [extents.x, extents.y],
                rect_size: [complete_width, extents.height],
                rect_color: complete_color,
                border_radius,
                border_size,
                border_color,
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
                    // Zero out left corners when partially complete
                    [base_radius[0], base_radius[1], 0.0, 0.0]
                } else {
                    base_radius
                };

                instances.push(shape_renderer::ShapeInstance {
                    rect_pos: [extents.x + complete_width, extents.y],
                    rect_size: [incomplete_width, extents.height],
                    rect_color: incomplete_color,
                    border_radius,
                    border_size,
                    border_color,
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
