use crate::{
    Urgency,
    components::{self, Bounds, Component},
    config::{self, Insets, Size, border::BorderRadius},
    rendering::texture_renderer,
    utils::buffers,
};
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

    fn get_text_areas(&self, _: Urgency) -> Vec<glyphon::TextArea<'_>> {
        vec![]
    }

    fn get_instances(&self, urgency: Urgency) -> Vec<buffers::Instance> {
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

            instances.push(buffers::Instance {
                rect_pos: [extents.x, extents.y],
                rect_size: [complete_width, extents.height],
                rect_color: style.complete_color.to_linear(urgency),
                border_radius: border_radius.into(),
                border_size: border_size.into(),
                border_color: style.border.color.to_linear(urgency),
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

                instances.push(buffers::Instance {
                    rect_pos: [extents.x + complete_width, extents.y],
                    rect_size: [incomplete_width, extents.height],
                    rect_color: style.incomplete_color.to_linear(urgency),
                    border_radius: border_radius.into(),
                    border_size: border_size.into(),
                    border_color: style.border.color.to_linear(urgency),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Urgency;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32},
    };

    fn create_test_progress(value: i32) -> Progress {
        let context = components::Context {
            id: 1,
            app_name: Arc::from("test_app"),
            config: Arc::new(crate::Config::default()),
            ui_state: crate::manager::UiState::default(),
        };
        let mut progress = Progress::new(context, value);
        progress.set_width(300.0);
        progress.set_position(0.0, 0.0);

        progress
    }

    #[test]
    fn test_initialization() {
        let progress = create_test_progress(50);

        assert_eq!(progress.get_id(), 1);
        assert_eq!(progress.value, 50);
        assert_eq!(progress.x, 0.0);
        assert_eq!(progress.y, 0.0);
        assert_eq!(progress.width, 300.0);
        assert_eq!(progress.get_app_name(), "test_app");
    }

    #[test]
    fn test_bounds_calculation() {
        let progress = create_test_progress(50);

        let bounds = progress.get_bounds();
        assert!(bounds.width > 0.0);
        assert!(bounds.height > 0.0);
    }

    #[test]
    fn test_render_bounds() {
        let progress = create_test_progress(50);

        let render_bounds = progress.get_render_bounds();
        assert!(render_bounds.width > 0.0);
        assert!(render_bounds.height > 0.0);
    }

    #[test]
    fn test_zero_progress() {
        let mut progress = create_test_progress(0);
        let width = 300.0;
        progress.set_width(width);

        let instances = progress.get_instances(Urgency::Normal);

        assert!(!instances.is_empty());

        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].rect_size[0], width);
    }

    #[test]
    fn test_full_progress() {
        let mut progress = create_test_progress(100);
        let width = 300.0;
        progress.set_width(width);

        let instances = progress.get_instances(Urgency::Normal);

        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].rect_size[0], width);
    }

    #[test]
    fn test_partial_progress() {
        let percentage = 50;
        let mut progress = create_test_progress(percentage);
        let width = 300.0;
        progress.set_width(width);

        let instances = progress.get_instances(Urgency::Normal);

        assert_eq!(instances.len(), 2);

        let expected_complete_width = (percentage as f32 / 100.0) * width;
        assert_eq!(instances[0].rect_size[0], expected_complete_width);

        let expected_incomplete_width = width - expected_complete_width;
        assert_eq!(instances[1].rect_size[0], expected_incomplete_width);

        let total_width: f32 = instances.iter().map(|instance| instance.rect_size[0]).sum();
        let render_bounds = progress.get_render_bounds();
        assert!((total_width - render_bounds.width).abs() < 0.001);
    }

    #[test]
    fn test_progress_over_100_percent() {
        let mut progress = create_test_progress(120);
        let width = 300.0;
        progress.set_width(width);

        let instances = progress.get_instances(Urgency::Normal);

        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].rect_size[0], width);
    }

    #[test]
    fn test_progress_negative_value() {
        let mut progress = create_test_progress(-20);
        let width = 300.0;
        progress.set_width(width);

        let instances = progress.get_instances(Urgency::Normal);

        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].rect_size[0], width);
    }

    #[test]
    fn test_low_progress() {
        let percentage = 25;
        let mut progress = create_test_progress(percentage);
        let width = 300.0;
        progress.set_width(width);

        let instances = progress.get_instances(Urgency::Normal);

        assert_eq!(instances.len(), 2);

        let expected_complete_width = (percentage as f32 / 100.0) * width;
        assert_eq!(instances[0].rect_size[0], expected_complete_width);

        let expected_incomplete_width = width - expected_complete_width;
        assert_eq!(instances[1].rect_size[0], expected_incomplete_width);
    }

    #[test]
    fn test_high_progress() {
        let percentage = 75;
        let mut progress = create_test_progress(percentage);
        let width = 300.0;
        progress.set_width(width);

        let instances = progress.get_instances(Urgency::Normal);

        assert_eq!(instances.len(), 2);

        let expected_complete_width = (percentage as f32 / 100.0) * width;
        assert_eq!(instances[0].rect_size[0], expected_complete_width);

        let expected_incomplete_width = width - expected_complete_width;
        assert_eq!(instances[1].rect_size[0], expected_incomplete_width);
    }

    #[test]
    fn test_almost_complete_progress() {
        let percentage = 99;
        let mut progress = create_test_progress(percentage);
        let width = 300.0;
        progress.set_width(width);

        let instances = progress.get_instances(Urgency::Normal);

        assert_eq!(instances.len(), 2);

        let expected_complete_width = (percentage as f32 / 100.0) * width;
        assert_eq!(instances[0].rect_size[0], expected_complete_width);

        let expected_incomplete_width = width - expected_complete_width;
        assert_eq!(instances[1].rect_size[0], expected_incomplete_width);
    }

    #[test]
    fn test_selection_state() {
        let context = components::Context {
            id: 1,
            app_name: Arc::from("test_app"),
            ui_state: crate::manager::UiState {
                selected_id: Arc::new(AtomicU32::new(1)),
                selected: Arc::new(AtomicBool::new(true)),
                ..Default::default()
            },
            config: Arc::new(crate::Config::default()),
        };
        let progress = Progress::new(context, 50);

        assert!(progress.get_ui_state().selected.load(Ordering::Relaxed));
        assert_eq!(
            progress.get_ui_state().selected_id.load(Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn test_set_width() {
        let mut progress = create_test_progress(50);

        progress.set_width(400.0);

        assert_eq!(progress.width, 400.0);
    }

    #[test]
    fn test_set_position() {
        let mut progress = create_test_progress(50);

        progress.set_position(10.0, 20.0);

        assert_eq!(progress.x, 10.0);
        assert_eq!(progress.y, 20.0);
    }
}
