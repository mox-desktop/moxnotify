use crate::{
    config::Config,
    rendering::{shape_renderer, text_renderer, texture_renderer},
    utils::buffers::{self, DepthBuffer},
    wgpu_state::WgpuState,
};
use anyhow::Context;
use raw_window_handle::{RawWindowHandle, WaylandWindowHandle};

use std::ptr::NonNull;
use wayland_client::{Proxy, protocol::wl_surface};

pub struct WgpuSurface {
    pub texture_renderer: texture_renderer::TextureRenderer,
    pub shape_renderer: shape_renderer::ShapeRenderer,
    pub text_ctx: text_renderer::TextContext,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub depth_buffer: buffers::DepthBuffer,
}

impl WgpuSurface {
    pub fn new(
        wgpu_state: &WgpuState,
        surface: &wl_surface::WlSurface,
        config: &Config,
    ) -> anyhow::Result<Self> {
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(surface.id().as_ptr().cast()).context("Surface id is a null ptr")?,
        ));

        let wgpu_surface = unsafe {
            wgpu_state
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: wgpu_state.raw_display_handle,
                    raw_window_handle,
                })?
        };

        let surface_caps = wgpu_surface.get_capabilities(&wgpu_state.adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .unwrap_or(&surface_caps.formats[0]);

        let alpha_mode = surface_caps
            .alpha_modes
            .iter()
            .find(|a| **a == wgpu::CompositeAlphaMode::PreMultiplied)
            .unwrap_or(&surface_caps.alpha_modes[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *surface_format,
            width: 1,
            height: 1,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: *alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let texture_renderer = texture_renderer::TextureRenderer::new(
            &wgpu_state.device,
            *surface_format,
            config.general.icon_size,
        );

        let shape_renderer =
            shape_renderer::ShapeRenderer::new(&wgpu_state.device, *surface_format);

        let text_ctx = text_renderer::TextContext::new(
            &wgpu_state.device,
            &wgpu_state.queue,
            surface_config.format,
        );

        let depth_buffer = DepthBuffer::new(&wgpu_state.device, 1, 1);

        Ok(Self {
            shape_renderer,
            texture_renderer,
            text_ctx,
            surface: wgpu_surface,
            config: surface_config,
            depth_buffer,
        })
    }
}
