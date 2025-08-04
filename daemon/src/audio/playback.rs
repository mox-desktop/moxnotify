use pipewire as pw;
use pw::{properties::properties, spa};
use spa::pod::Pod;
use std::{fs, path::Path, time::Duration};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{CODEC_TYPE_NULL, DecoderOptions},
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

pub struct Ready;
pub struct Played;

#[derive(Clone)]
pub struct Data {
    buffer: Vec<f32>,
    channels_count: usize,
    sample_rate: usize,
    position: usize,
}

pub struct Playback<State = Ready> {
    thread_loop: pw::thread_loop::ThreadLoop,
    stream: pw::stream::Stream,
    duration: Duration,
    _state: State,
    data: Data,
    _listener: Option<pw::stream::StreamListener<Data>>,
    pub cooldown: Option<std::time::Instant>,
}

impl Playback {
    pub fn new<T>(
        threadloop: pw::thread_loop::ThreadLoop,
        core: &pw::core::Core,
        path: T,
    ) -> anyhow::Result<Playback<Ready>>
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
            .ok_or(anyhow::anyhow!("No valid track"))?;

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
        .unwrap_or(Duration::from_secs(1));

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
            audio_buffer.extend_from_slice(samples);
        }

        let lock = threadloop.lock();
        let stream = pw::stream::Stream::new(
            core,
            "audio-playback",
            properties! {
                *pw::keys::MEDIA_TYPE => "Audio",
                *pw::keys::MEDIA_ROLE => "Event",
                *pw::keys::MEDIA_CATEGORY => "Playback",
                *pw::keys::AUDIO_CHANNELS => "2",
            },
        )?;
        lock.unlock();

        Ok(Self {
            stream,
            duration,
            _state: Ready,
            thread_loop: threadloop,
            data: Data {
                buffer: audio_buffer,
                channels_count,
                sample_rate,
                position: 0,
            },
            _listener: None,
            cooldown: None,
        })
    }

    pub fn start(self) -> Playback<Played> {
        let lock = self.thread_loop.lock();

        let listener = self
            .stream
            .add_local_listener_with_user_data(self.data.clone())
            .process(|stream, user_data| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };

                let datas = buffer.datas_mut();
                let stride = std::mem::size_of::<i16>() * user_data.channels_count;
                let data = &mut datas[0];

                let n_frames = if let Some(slice) = data.data() {
                    let n_frames = slice.len() / stride;

                    for i in 0..n_frames {
                        for c in 0..user_data.channels_count {
                            let sample_index = user_data.position + c;
                            let sample = if sample_index < user_data.buffer.len() {
                                (user_data.buffer[sample_index].clamp(-1.0, 1.0) * i16::MAX as f32)
                                    as i16
                            } else {
                                0
                            };

                            let start = i * stride + (c * std::mem::size_of::<i16>());
                            let end = start + std::mem::size_of::<i16>();
                            let chan = &mut slice[start..end];
                            chan.copy_from_slice(&i16::to_le_bytes(sample));
                        }
                        user_data.position += user_data.channels_count;
                    }
                    n_frames
                } else {
                    0
                };

                let chunk = data.chunk_mut();
                *chunk.offset_mut() = 0;
                *chunk.stride_mut() = stride as _;
                *chunk.size_mut() = (stride * n_frames) as _;
            })
            .register()
            .unwrap();

        let mut audio_info = spa::param::audio::AudioInfoRaw::new();
        audio_info.set_format(spa::param::audio::AudioFormat::S16LE);
        audio_info.set_rate(self.data.sample_rate as u32);
        audio_info.set_channels(self.data.channels_count as u32);
        let mut position = [0; spa::param::audio::MAX_CHANNELS];
        position[0] = libspa_sys::SPA_AUDIO_CHANNEL_FL;
        position[1] = libspa_sys::SPA_AUDIO_CHANNEL_FR;
        audio_info.set_position(position);

        let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &pw::spa::pod::Value::Object(pw::spa::pod::Object {
                type_: libspa_sys::SPA_TYPE_OBJECT_Format,
                id: libspa_sys::SPA_PARAM_EnumFormat,
                properties: audio_info.into(),
            }),
        )
        .unwrap()
        .0
        .into_inner();

        let mut params = [Pod::from_bytes(&values).unwrap()];

        self.stream
            .connect(
                spa::utils::Direction::Output,
                None,
                pw::stream::StreamFlags::AUTOCONNECT
                    | pw::stream::StreamFlags::MAP_BUFFERS
                    | pw::stream::StreamFlags::RT_PROCESS,
                &mut params,
            )
            .unwrap();

        lock.unlock();

        Playback {
            thread_loop: self.thread_loop,
            stream: self.stream,
            duration: self.duration,
            _state: Played,
            data: self.data,
            _listener: Some(listener),
            cooldown: Some(std::time::Instant::now()),
        }
    }
}

impl Playback<Played> {
    pub fn stop(self) {
        self.stream.disconnect().unwrap();
    }
}
