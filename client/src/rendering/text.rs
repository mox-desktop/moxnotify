use crate::components::Bounds;
use crate::styles::Font;
use glyphon::{Attrs, Buffer, FontSystem, Shaping, Weight};

fn create_buffer(font: &Font, font_system: &mut FontSystem, max_width: Option<f32>) -> Buffer {
    let dpi = 96.0;
    let font_size = font.size as f32 * dpi / 72.0;
    let mut buffer = Buffer::new(
        font_system,
        glyphon::Metrics::new(font_size, font_size * 1.2),
    );
    buffer.shape_until_scroll(font_system, true);
    buffer.set_size(font_system, max_width, None);
    buffer
}

pub struct Text {
    pub buffer: Buffer,
    x: f32,
    y: f32,
}

impl Text {
    pub fn new<T>(font: &Font, font_system: &mut FontSystem, body: T) -> Self
    where
        T: AsRef<str>,
    {
        let attrs = Attrs::new()
            .metadata(0.6_f32.to_bits() as usize)
            .family(glyphon::Family::Name(&font.family))
            .weight(Weight::BOLD);
        let mut buffer = create_buffer(font, font_system, None);
        buffer.set_text(font_system, body.as_ref(), &attrs, Shaping::Advanced, None);

        Self {
            buffer,
            x: 0.,
            y: 0.,
        }
    }

    pub fn set_buffer_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    pub fn get_bounds(&self) -> Bounds {
        let (width, total_lines) = self
            .buffer
            .layout_runs()
            .fold((0.0, 0.0), |(width, total_lines), run| {
                (run.line_w.max(width), total_lines + 1.0)
            });

        Bounds {
            x: self.x,
            y: self.y,
            width,
            height: total_lines * self.buffer.metrics().line_height,
        }
    }
}
