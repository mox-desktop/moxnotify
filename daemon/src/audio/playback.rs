use crate::audio::tinyaudio::SoundDevice;

use super::tinyaudio::OutputDeviceParameters;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Duration,
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

#[derive(Clone)]
pub struct Ready;

pub struct Played {
    shutdown_channel: (
        crossbeam_channel::Sender<()>,
        crossbeam_channel::Receiver<()>,
    ),
}

#[derive(Clone)]
pub struct Playback<State = Ready> {
    duration: Duration,
    buffer: Vec<f32>,
    params: OutputDeviceParameters,
    state: State,
}

impl Playback {
    pub fn new<T>(path: T) -> anyhow::Result<Playback<Ready>>
    where
        T: AsRef<Path>,
    {
        let src = fs::File::open(&path)?;
        let mss = MediaSourceStream::new(Box::new(src), Default::default());
        let hint = Hint::new();

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| anyhow::anyhow!("Failed to probe audio format: {}", e))?;

        let track = probed
            .format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(anyhow::anyhow!("No valid audio track found"))?;

        let channels_count = track
            .codec_params
            .channels
            .map(|channels| channels.count())
            .ok_or(anyhow::anyhow!("Unable to determine channel count"))?;

        let sample_rate = track
            .codec_params
            .sample_rate
            .ok_or(anyhow::anyhow!("Unable to determine sample rate"))?
            as usize;

        let mut format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(anyhow::anyhow!(""))?;

        let dec_opts = DecoderOptions::default();
        let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

        let mut audio_buffer: Vec<f32> = Vec::new();

        let duration = if let Some(time_base) = track.codec_params.time_base {
            if let Some(n_frames) = track.codec_params.n_frames {
                let duration_seconds =
                    (n_frames as f64) / (time_base.denom as f64 / time_base.numer as f64);
                Some(std::time::Duration::from_secs_f64(duration_seconds))
            } else {
                None
            }
        } else {
            None
        }
        .unwrap();

        let track_id = track.id;
        while let Ok(packet) = format.next_packet() {
            while !format.metadata().is_latest() {
                format.metadata().pop();
            }
            if packet.track_id() != track_id {
                continue;
            }
            let decoded = decoder.decode(&packet)?;
            let mut sample_buf =
                SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
            sample_buf.copy_interleaved_ref(decoded);
            let samples: &[f32] = bytemuck::cast_slice(sample_buf.samples());

            {
                let buffer = &mut audio_buffer;
                samples.iter().for_each(|sample| {
                    buffer.push(*sample);
                });
            }
        }

        let params = OutputDeviceParameters {
            channels_count,
            sample_rate,
            channel_sample_count: audio_buffer.len(),
        };

        Ok(Self {
            duration,
            buffer: audio_buffer,
            params,
            state: Ready,
        })
    }

    pub fn start(self) -> Playback<Played> {
        let (tx, rx) = crossbeam_channel::unbounded();

        let buffer = self.buffer.clone();
        let params = self.params;
        let duration = self.duration;

        thread::spawn({
            let rx = rx.clone();
            move || {
                let index = AtomicUsize::new(0);
                let mut device = SoundDevice::new(params, move |data| {
                    data.iter_mut().for_each(|sample| {
                        let current_index = index.fetch_add(1, Ordering::Relaxed);
                        *sample = *buffer.get(current_index).unwrap_or(&0.0);
                    });
                })
                .unwrap();

                device.run().unwrap();
                let _ = rx.recv_timeout(duration);
                device.stop().unwrap();
            }
        });

        Playback {
            duration: self.duration,
            buffer: self.buffer,
            params: self.params,
            state: Played {
                shutdown_channel: (tx, rx),
            },
        }
    }
}

impl Playback<Played> {
    pub fn stop(self) {
        self.state.shutdown_channel.0.send(()).unwrap();
    }
}
