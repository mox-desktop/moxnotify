use crate::{
    Urgency,
    components::{self, Component},
    config::{self, Insets, Size, border::BorderRadius},
    rendering::texture_renderer,
    utils::{buffers, taffy::GlobalLayout},
};
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

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
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

    fn apply_computed_layout(&mut self, tree: &mut taffy::TaffyTree<()>) {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        self.x = layout.location.x;
        self.y = layout.location.y;
    }

    fn get_text_areas(&self, _: &taffy::TaffyTree<()>, _: Urgency) -> Vec<glyphon::TextArea<'_>> {
        Vec::new()
    }

    fn get_instances(
        &self,
        tree: &taffy::TaffyTree<()>,
        urgency: Urgency,
    ) -> Vec<buffers::Instance> {
        let bounds = self.get_render_bounds(tree);

        let progress_ratio = (self.value as f32 / 100.0).min(1.0);

        let mut instances = Vec::new();
        let complete_width = (bounds.width * progress_ratio).max(0.);

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
                rect_pos: [bounds.x, bounds.y],
                rect_size: [complete_width, bounds.height],
                rect_color: style.complete_color.color(urgency),
                border_radius: border_radius.into(),
                border_size: border_size.into(),
                border_color: style.border.color.color(urgency),
                scale: self.get_ui_state().scale.load(Ordering::Relaxed),
                depth: 0.8,
            });
        }

        if self.value < 100 {
            let incomplete_width = bounds.width - complete_width;

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
                    rect_pos: [bounds.x + complete_width, bounds.y],
                    rect_size: [incomplete_width, bounds.height],
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

    fn get_textures(&self, tree: &taffy::TaffyTree<()>) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }

    fn get_node_id(&self) -> taffy::NodeId {
        self.node
    }
}

impl Progress {
    #[must_use]
    pub fn new(tree: &mut taffy::TaffyTree, context: components::Context, value: i32) -> Self {
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
        progress.update_layout(0.0, 0.0);

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

        progress.update_layout(10.0, 20.0);

        assert_eq!(progress.x, 10.0);
        assert_eq!(progress.y, 20.0);
    }
}
