use crate::utils::{
    buffers::{self, Buffer, DataDescription},
    math::{Mat4, Matrix},
};

pub struct ShapeRenderer {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: buffers::VertexBuffer,
    index_buffer: buffers::IndexBuffer,
    instance_buffer: buffers::InstanceBuffer<buffers::Instance>,
    projection_uniform: buffers::Projection,
}

impl ShapeRenderer {
    pub fn new(device: &wgpu::Device, texture_format: wgpu::TextureFormat) -> Self {
        let projection_uniform = buffers::Projection::new(device, 0.0, 0.0, 0.0, 0.0);

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&projection_uniform.bind_group_layout],
                push_constant_ranges: &[],
            });

        let shader = device.create_shader_module(wgpu::include_wgsl!("./shader.wgsl"));
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[buffers::Vertex::desc(), buffers::Instance::desc()],
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multiview: None,
            cache: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        });

        let index_buffer = buffers::IndexBuffer::new(device, &[0, 1, 3, 1, 2, 3]);

        let vertex_buffer = buffers::VertexBuffer::new(
            device,
            &[
                buffers::Vertex {
                    position: [0.0, 1.0],
                },
                buffers::Vertex {
                    position: [1.0, 1.0],
                },
                buffers::Vertex {
                    position: [1.0, 0.0],
                },
                buffers::Vertex {
                    position: [0.0, 0.0],
                },
            ],
        );

        let instance_buffer = buffers::InstanceBuffer::new(device, &[]);

        Self {
            render_pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            projection_uniform,
        }
    }

    pub fn resize(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let projection = Mat4::projection(0.0, width, 0.0, height);

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
        instances: &[buffers::Instance],
    ) {
        if instances.is_empty() {
            return;
        }

        let needed_buffer_size = std::mem::size_of_val(instances);

        if self.instance_buffer.size() < needed_buffer_size as u32 {
            self.instance_buffer =
                buffers::InstanceBuffer::with_size(device, needed_buffer_size as u64);
        }

        self.instance_buffer.write(queue, instances);
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.projection_uniform.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(
            0..self.index_buffer.size(),
            0,
            0..self.instance_buffer.size(),
        );
    }
}
