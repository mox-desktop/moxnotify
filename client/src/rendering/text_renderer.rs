use crate::components::Bounds;
use config::client::Font;
use glyphon::{
    Attrs, Buffer, Cache, FontSystem, Shaping, SwashCache, TextArea, TextAtlas, TextRenderer,
    Viewport, Weight,
};
use wgpu::{MultisampleState, TextureFormat};

fn create_buffer(font: &Font, font_system: &mut FontSystem, max_width: Option<f32>) -> Buffer {
    let dpi = 96.0;
    let font_size = font.size * dpi / 72.0;
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

pub struct TextContext {
    pub swash_cache: glyphon::SwashCache,
    pub viewport: glyphon::Viewport,
    pub atlas: glyphon::TextAtlas,
    pub renderer: glyphon::TextRenderer,
}

impl TextContext {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, texture_format: TextureFormat) -> Self {
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, texture_format);
        let renderer = TextRenderer::new(
            &mut atlas,
            device,
            MultisampleState::default(),
            Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
        );

        Self {
            swash_cache,
            viewport: Viewport::new(device, &cache),
            atlas,
            renderer,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text: Vec<TextArea>,
        font_system: &mut FontSystem,
    ) -> anyhow::Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        self.renderer.prepare_with_depth(
            device,
            queue,
            font_system,
            &mut self.atlas,
            &self.viewport,
            text,
            &mut self.swash_cache,
            |metadata| f32::from_bits(metadata as u32),
        )?;

        Ok(())
    }

    pub fn render(&mut self, render_pass: &mut wgpu::RenderPass) -> anyhow::Result<()> {
        self.renderer
            .render(&self.atlas, &self.viewport, render_pass)?;

        Ok(())
    }
}
