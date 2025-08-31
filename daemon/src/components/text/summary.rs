use super::Text;
use crate::{
    Urgency,
    components::{self, Bounds, Component, Data},
    config,
    rendering::texture_renderer,
    utils::{
        buffers,
        taffy::{GlobalLayout, NodeContext},
    },
};
use glyphon::{Attrs, Buffer, FontSystem, Weight};
use std::sync::{Arc, atomic::Ordering};
use taffy::style_helpers::{auto, length, line};

pub struct Summary {
    node: taffy::NodeId,
    context: components::Context,
    pub buffer: Buffer,
    x: f32,
    y: f32,
}

impl Text for Summary {
    fn set_size(&mut self, font_system: &mut FontSystem, width: Option<f32>, height: Option<f32>) {
        self.buffer.set_size(font_system, width, height);
    }

    fn set_text<T>(&mut self, font_system: &mut FontSystem, text: T)
    where
        T: AsRef<str>,
    {
        let style = &self.get_style();
        let family = Arc::clone(&style.family);

        let attrs = Attrs::new()
            .metadata(0.7_f32.to_bits() as usize)
            .family(glyphon::Family::Name(&family))
            .weight(Weight::BOLD);

        self.buffer.set_text(
            font_system,
            text.as_ref(),
            &attrs,
            glyphon::Shaping::Advanced,
        );
    }
}

impl Component for Summary {
    type Style = config::text::Summary;

    fn get_context(&self) -> &components::Context {
        &self.context
    }

    fn get_style(&self) -> &Self::Style {
        &self.get_notification_style().summary
    }

    fn get_instances(
        &self,
        tree: &taffy::TaffyTree<NodeContext>,
        urgency: Urgency,
    ) -> Vec<buffers::Instance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds(tree);

        vec![buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
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
        urgency: crate::Urgency,
    ) -> Vec<glyphon::TextArea<'_>> {
        let style = self.get_style();
        let bounds = self.get_render_bounds(tree);

        if bounds.width == 0. {
            return Vec::new();
        }

        let content_width = bounds.width
            - style.border.size.left
            - style.border.size.right
            - style.padding.left
            - style.padding.right;

        let content_height = bounds.height
            - style.border.size.top
            - style.border.size.bottom
            - style.padding.top
            - style.padding.bottom;

        let left = bounds.x + style.border.size.left + style.padding.left;
        let top = bounds.y + style.border.size.top + style.padding.top;

        vec![glyphon::TextArea {
            buffer: &self.buffer,
            left,
            top,
            scale: self.get_ui_state().scale.load(Ordering::Relaxed),
            bounds: glyphon::TextBounds {
                left: left as i32,
                top: top as i32,
                right: (left + content_width) as i32,
                bottom: (top + content_height) as i32,
            },
            default_color: style.color.into_glyphon(urgency),
            custom_glyphs: &[],
        }]
    }

    fn get_textures(
        &self,
        _: &taffy::TaffyTree<NodeContext>,
    ) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }

    fn get_bounds(&self, _: &taffy::TaffyTree<NodeContext>) -> Bounds {
        let style = self.get_style();
        let (width, total_lines) = self
            .buffer
            .layout_runs()
            .fold((0.0, 0.0), |(width, total_lines), run| {
                (run.line_w.max(width), total_lines + 1.0)
            });

        if width == 0. || total_lines == 0. {
            return Bounds {
                x: 0.,
                y: 0.,
                width: 0.,
                height: 0.,
            };
        }

        Bounds {
            x: self.x,
            y: self.y,
            width: width
                + style.margin.left
                + style.margin.right
                + style.padding.left
                + style.padding.right
                + style.border.size.left
                + style.border.size.right,
            height: total_lines * self.buffer.metrics().line_height
                + style.margin.top
                + style.margin.bottom
                + style.padding.top
                + style.padding.bottom
                + style.border.size.top
                + style.border.size.bottom,
        }
    }

    fn get_render_bounds(&self, tree: &taffy::TaffyTree<NodeContext>) -> Bounds {
        let style = self.get_style();
        let bounds = self.get_bounds(tree);

        Bounds {
            x: bounds.x + style.margin.left,
            y: bounds.y + style.margin.top,
            width: bounds.width - style.margin.left - style.margin.right,
            height: bounds.height - style.margin.top - style.margin.bottom,
        }
    }

    fn update_layout(&mut self, tree: &mut taffy::TaffyTree<NodeContext>) {
        let style = self.get_style();
        let summary_size = self.get_render_bounds(tree);

        self.node = tree
            .new_leaf(taffy::Style {
                grid_row: line(1),
                grid_column: line(2),
                size: taffy::Size {
                    width: auto(),
                    height: length(summary_size.height),
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
    }

    fn apply_computed_layout(&mut self, tree: &taffy::TaffyTree<NodeContext>) {
        let layout = tree.global_layout(self.get_node_id()).unwrap();
        self.x = layout.location.x;
        self.y = layout.location.y;
    }

    fn get_data(&self, tree: &taffy::TaffyTree<NodeContext>, urgency: Urgency) -> Vec<Data<'_>> {
        self.get_instances(tree, urgency)
            .into_iter()
            .map(Data::Instance)
            .chain(
                self.get_text_areas(tree, urgency)
                    .into_iter()
                    .map(Data::TextArea),
            )
            .collect()
    }

    fn get_node_id(&self) -> taffy::NodeId {
        self.node
    }
}

impl Summary {
    pub fn new(
        tree: &mut taffy::TaffyTree<NodeContext>,
        context: components::Context,
        font_system: &mut FontSystem,
    ) -> Self {
        let dpi = 96.0;
        let font_size = context.config.styles.default.font.size * dpi / 72.0;
        let mut buffer = Buffer::new(
            font_system,
            glyphon::Metrics::new(font_size, font_size * 1.2),
        );
        buffer.shape_until_scroll(font_system, true);
        buffer.set_size(font_system, None, None);

        let node = tree.new_leaf(taffy::Style::DEFAULT).unwrap();

        Self {
            buffer,
            x: 0.,
            y: 0.,
            context,
            node,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        components::{
            self,
            text::{Text, summary::Summary},
        },
        config::Config,
        manager::UiState,
    };
    use glyphon::FontSystem;
    use std::sync::Arc;

    #[test]
    fn test_body() {
        let mut font_system = FontSystem::new();

        let context = components::Context {
            id: 0,
            config: Arc::new(Config::default()),
            app_name: "".into(),
            ui_state: UiState::default(),
        };
        let mut summary = Summary::new(context, &mut font_system);

        summary.set_text(
            &mut font_system,
            "Hello world\n<b>Hello world</b>\n<i>Hello world</i>",
        );

        let lines = summary.buffer.lines;
        assert_eq!(lines.first().unwrap().text(), "Hello world");
        assert_eq!(lines.get(1).unwrap().text(), "<b>Hello world</b>");
        assert_eq!(lines.get(2).unwrap().text(), "<i>Hello world</i>");
        assert_eq!(lines.len(), 3);
    }
}
