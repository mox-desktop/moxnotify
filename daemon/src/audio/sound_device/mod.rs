mod device;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

#[derive(Copy, Clone)]
pub struct OutputDeviceParameters {
    pub sample_rate: usize,
    pub channels_count: usize,
    pub channel_sample_count: usize,
}

pub struct SoundDevice {
    params: OutputDeviceParameters,
    playback_device: Arc<device::Device>,
    thread_handle: Option<JoinHandle<()>>,
    is_running: Arc<AtomicBool>,
}

impl SoundDevice {
    pub fn new(params: OutputDeviceParameters) -> anyhow::Result<Self> {
        let is_running = Arc::new(AtomicBool::new(false));

        Ok(Self {
            playback_device: Arc::new(device::Device::new("default", &params)?),
            is_running,
            thread_handle: None,
            params,
        })
    }

    pub fn run<C>(&mut self, data_callback: C) -> anyhow::Result<()>
    where
        C: FnMut(&mut [f32]) + Send + 'static,
    {
        self.is_running.store(true, Ordering::SeqCst);

        let thread_handle = DataSender {
            playback_device: Arc::clone(&self.playback_device),
            callback: data_callback,
            data_buffer: vec![
                0.0f32;
                self.params.channel_sample_count * self.params.channels_count
            ],
            output_buffer: vec![
                0i16;
                self.params.channel_sample_count * self.params.channels_count
            ],
            is_running: self.is_running.clone(),
            params: self.params,
        }
        .run_in_thread()?;

        self.thread_handle = Some(thread_handle);

        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(thread_handle) = self.thread_handle.take() {
            thread_handle.join().unwrap();
        }

        Ok(())
    }
}

impl Drop for SoundDevice {
    fn drop(&mut self) {
        self.stop().unwrap();
    }
}

struct DataSender<C> {
    playback_device: Arc<device::Device>,
    callback: C,
    data_buffer: Vec<f32>,
    output_buffer: Vec<i16>,
    is_running: Arc<AtomicBool>,
    params: OutputDeviceParameters,
}

unsafe impl<C> Send for DataSender<C> {}

impl<C> DataSender<C>
where
    C: FnMut(&mut [f32]) + Send + 'static,
{
    pub fn run_in_thread(mut self) -> anyhow::Result<JoinHandle<()>> {
        Ok(std::thread::Builder::new()
            .name("AlsaDataSender".to_string())
            .spawn(move || self.run_send_loop())?)
    }

    pub fn run_send_loop(&mut self) {
        while self.is_running.load(Ordering::SeqCst) {
            (self.callback)(&mut self.data_buffer);

            debug_assert_eq!(self.data_buffer.len(), self.output_buffer.len());
            for (in_sample, out_sample) in
                self.data_buffer.iter().zip(self.output_buffer.iter_mut())
            {
                *out_sample = (*in_sample * i16::MAX as f32) as i16;
            }

            _ = self
                .playback_device
                .writei(&self.output_buffer, self.params.channel_sample_count);
        }
    }
}
