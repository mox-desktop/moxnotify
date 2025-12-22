use super::Text;
use crate::{
    components::{self, Bounds, Component, Data},
    config,
    moxnotify::common::Urgency,
};
use glyphon::{Attrs, Buffer, FontSystem, Weight};
use moxui::{shape_renderer, texture_renderer};
use std::sync::{Arc, atomic::Ordering};

pub struct Summary {
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

    fn get_instances(&self, urgency: Urgency) -> Vec<shape_renderer::ShapeInstance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds();

        vec![shape_renderer::ShapeInstance {
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

    fn get_text_areas(&self, urgency: Urgency) -> Vec<glyphon::TextArea<'_>> {
        let style = self.get_style();
        let bounds = self.get_render_bounds();

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

    fn get_textures(&self) -> Vec<texture_renderer::TextureArea<'_>> {
        Vec::new()
    }

    fn get_bounds(&self) -> Bounds {
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

    fn get_render_bounds(&self) -> Bounds {
        let style = self.get_style();
        let bounds = self.get_bounds();
        Bounds {
            x: bounds.x + style.margin.left,
            y: bounds.y + style.margin.top,
            width: bounds.width - style.margin.left - style.margin.right,
            height: bounds.height - style.margin.top - style.margin.bottom,
        }
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn get_data(&self, urgency: Urgency) -> Vec<Data<'_>> {
        self.get_instances(urgency)
            .into_iter()
            .map(Data::Instance)
            .chain(self.get_text_areas(urgency).into_iter().map(Data::TextArea))
            .collect()
    }
}

impl Summary {
    pub fn new(context: components::Context, font_system: &mut FontSystem) -> Self {
        let dpi = 96.0;
        let font_size = context.config.styles.default.font.size * dpi / 72.0;
        let mut buffer = Buffer::new(
            font_system,
            glyphon::Metrics::new(font_size, font_size * 1.2),
        );
        buffer.shape_until_scroll(font_system, true);
        buffer.set_size(font_system, None, None);

        Self {
            buffer,
            x: 0.,
            y: 0.,
            context,
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
