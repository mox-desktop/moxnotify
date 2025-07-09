use raw_window_handle::{RawDisplayHandle, WaylandDisplayHandle};
use std::ptr::NonNull;
use wayland_client::Connection;

pub struct WgpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,
    pub raw_display_handle: RawDisplayHandle,
}

impl WgpuState {
    pub async fn new(conn: &Connection) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(conn.backend().display_ptr() as *mut _).unwrap(),
        ));

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("Failed to find suitable adapter");

        let (device, queue) = adapter
            .request_device(&Default::default())
            .await
            .expect("Failed to request device");

        Ok(Self {
            device,
            queue,
            instance,
            adapter,
            raw_display_handle,
        })
    }
}
