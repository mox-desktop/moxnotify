use crate::audio::sound_device::OutputDeviceParameters;
use alsa_sys::*;
use std::{
    ffi::{CStr, CString},
    os::raw::c_int,
};

pub fn err_code_to_string(err_code: c_int) -> String {
    unsafe {
        let message = CStr::from_ptr(snd_strerror(err_code) as *const _)
            .to_bytes()
            .to_vec();
        String::from_utf8(message).unwrap()
    }
}

pub fn check(err_code: c_int) -> anyhow::Result<()> {
    if err_code < 0 {
        Err(anyhow::anyhow!(err_code_to_string(err_code)))
    } else {
        Ok(())
    }
}

pub struct Device(*mut snd_pcm_t);

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Device {
    pub fn new(name: &str, params: &OutputDeviceParameters) -> anyhow::Result<Self> {
        let mut playback_device = std::ptr::null_mut();
        let name = CString::new(name).unwrap();
        let frame_count = params.channel_sample_count;
        unsafe {
            check(snd_pcm_open(
                &mut playback_device,
                name.as_ptr() as *const _,
                SND_PCM_STREAM_PLAYBACK,
                0,
            ))?;
        }

        let mut hw_params = std::ptr::null_mut();
        unsafe {
            check(snd_pcm_hw_params_malloc(&mut hw_params))?;
            check(snd_pcm_hw_params_any(playback_device, hw_params))?;
        }

        let access = SND_PCM_ACCESS_RW_INTERLEAVED;
        unsafe {
            check(snd_pcm_hw_params_set_access(
                playback_device,
                hw_params,
                access,
            ))?;
            check(snd_pcm_hw_params_set_format(
                playback_device,
                hw_params,
                SND_PCM_FORMAT_S16_LE,
            ))?;
        }

        let mut exact_rate = params.sample_rate as ::std::os::raw::c_uint;
        unsafe {
            check(snd_pcm_hw_params_set_rate_near(
                playback_device,
                hw_params,
                &mut exact_rate,
                std::ptr::null_mut(),
            ))?;
            check(snd_pcm_hw_params_set_channels(
                playback_device,
                hw_params,
                params.channels_count as ::std::os::raw::c_uint,
            ))?;
        }

        let mut _exact_period = frame_count as snd_pcm_uframes_t;
        let mut _direction = 0;
        unsafe {
            check(snd_pcm_hw_params_set_period_size_near(
                playback_device,
                hw_params,
                &mut _exact_period,
                &mut _direction,
            ))?;
        }

        let mut exact_size = (frame_count * 2) as ::std::os::raw::c_ulong;
        unsafe {
            check(snd_pcm_hw_params_set_buffer_size_near(
                playback_device,
                hw_params,
                &mut exact_size,
            ))?;
            check(snd_pcm_hw_params(playback_device, hw_params))?;
            snd_pcm_hw_params_free(hw_params);
        }

        let mut sw_params = std::ptr::null_mut();
        unsafe {
            check(snd_pcm_sw_params_malloc(&mut sw_params))?;
            check(snd_pcm_sw_params_current(playback_device, sw_params))?;
            check(snd_pcm_sw_params_set_avail_min(
                playback_device,
                sw_params,
                frame_count as ::std::os::raw::c_ulong,
            ))?;
            check(snd_pcm_sw_params_set_start_threshold(
                playback_device,
                sw_params,
                frame_count as ::std::os::raw::c_ulong,
            ))?;
            check(snd_pcm_sw_params(playback_device, sw_params))?;
            check(snd_pcm_prepare(playback_device))?;
        }

        Ok(Self(playback_device))
    }

    pub fn writei(&self, output_buffer: &[i16], channel_sample_count: usize) -> anyhow::Result<()> {
        unsafe {
            let err = snd_pcm_writei(
                self.0,
                output_buffer.as_ptr() as *const _,
                channel_sample_count as ::std::os::raw::c_ulong,
            ) as i32;

            'try_loop: for _ in 0..10 {
                if err < 0 {
                    // Try to recover from error
                    snd_pcm_recover(self.0, err, 1);
                } else {
                    break 'try_loop;
                }
            }

            check(err)
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            snd_pcm_close(self.0);
        }
    }
}
