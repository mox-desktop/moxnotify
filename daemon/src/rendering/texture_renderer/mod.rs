use crate::utils::{
    buffers::{self, Buffer, DataDescription},
    math::{Mat4, Matrix},
};

pub struct TextureRenderer {
    render_pipeline: wgpu::RenderPipeline,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    vertex_buffer: buffers::VertexBuffer,
    index_buffer: buffers::IndexBuffer,
    projection_uniform: buffers::Projection,
    instance_buffer: buffers::InstanceBuffer<buffers::TextureInstance>,
    height: f32,
    max_icon_size: u32,
    prepared_instances: usize,
}

#[derive(Clone)]
pub struct TextureArea<'a> {
    pub left: f32,
    pub top: f32,
    pub bounds: TextureBounds,
    pub scale: f32,
    pub radius: [f32; 4],
    pub data: &'a [u8],
    pub width: f32,
    pub height: f32,
    pub border_size: [f32; 4],
    pub depth: f32,
}

#[derive(Clone)]
pub struct TextureBounds {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

impl TextureRenderer {
    pub fn new(
        device: &wgpu::Device,
        texture_format: wgpu::TextureFormat,
        max_icon_size: u32,
    ) -> Self {
        let projection_uniform = buffers::Projection::new(device, 0.0, 0.0, 0.0, 0.0);

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let texture_size = wgpu::Extent3d {
            width: max_icon_size,
            height: max_icon_size,
            depth_or_array_layers: 256,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture_renderer_texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_array_layer: 0,
            array_layer_count: Some(256),
            ..Default::default()
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("texture_renderer_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("texture_bind_group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("texture_render_pipeline_layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &projection_uniform.bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let shader = device.create_shader_module(wgpu::include_wgsl!("./shader.wgsl"));

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("texture_render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[buffers::Vertex::desc(), buffers::TextureInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = buffers::VertexBuffer::new(
            device,
            &[
                buffers::Vertex {
                    position: [0.0, 0.0],
                },
                buffers::Vertex {
                    position: [1.0, 0.0],
                },
                buffers::Vertex {
                    position: [0.0, 1.0],
                },
                buffers::Vertex {
                    position: [1.0, 1.0],
                },
            ],
        );

        let index_buffer = buffers::IndexBuffer::new(device, &[0, 1, 3, 3, 2, 0]);

        let instance_buffer = buffers::InstanceBuffer::new(device, &[]);

        Self {
            prepared_instances: 0,
            max_icon_size,
            instance_buffer,
            projection_uniform,
            render_pipeline,
            texture,
            index_buffer,
            vertex_buffer,
            bind_group,
            height: 0.,
        }
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, width: f32, height: f32) {
        // This is fucking pissing me off, for some reason the texture just disappears when I make
        // height bottom and 0.0 top and it forces me to hack a bit
        let projection = Mat4::projection(0.0, width, height, 0.0);

        self.height = height;

        queue.write_buffer(
            &self.projection_uniform.buffer,
            0,
            bytemuck::cast_slice(&projection),
        );
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: &[TextureArea],
    ) {
        self.prepared_instances = textures.len();

        if textures.is_empty() {
            return;
        }

        let mut instances = Vec::new();

        textures.iter().enumerate().for_each(|(i, texture)| {
            instances.push(buffers::TextureInstance {
                scale: texture.scale,
                pos: [texture.left, self.height - texture.top - texture.height],
                size: [texture.width, texture.height],
                radius: texture.radius,
                border_width: texture.border_size,
                container_rect: [
                    texture.bounds.left as f32,
                    self.height - texture.bounds.top as f32 - texture.height,
                    texture.bounds.right as f32,
                    self.height - texture.bounds.bottom as f32,
                ],
                depth: texture.depth,
            });

            let bytes_per_row = (4 * self.max_icon_size).div_ceil(256) * 256;

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                texture.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: self.max_icon_size,
                    height: self.max_icon_size,
                    depth_or_array_layers: 1,
                },
            );
        });

        let instance_buffer_size =
            std::mem::size_of::<buffers::TextureInstance>() * instances.len();

        if self.instance_buffer.size() < instance_buffer_size as u32 {
            self.instance_buffer =
                buffers::InstanceBuffer::with_size(device, instance_buffer_size as u64);
        }

        self.instance_buffer.write(queue, &instances);
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass) {
        if self.prepared_instances == 0 {
            return;
        }

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_bind_group(1, &self.projection_uniform.bind_group, &[]);

        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        render_pass.draw_indexed(
            0..self.index_buffer.size(),
            0,
            0..self.prepared_instances as u32,
        );
    }
}
