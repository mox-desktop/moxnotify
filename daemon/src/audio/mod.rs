mod playback;

use pipewire::{self as pw, sys::PW_ID_CORE};
use std::path::Path;

pub struct Audio {
    muted: bool,
    playback: Option<playback::Playback<playback::Played>>,
    thread_loop: pw::thread_loop::ThreadLoop,
    context: pw::context::Context,
    core: pw::core::Core,
}

impl Audio {
    pub fn try_new() -> anyhow::Result<Self> {
        pw::init();
        let thread_loop = unsafe { pw::thread_loop::ThreadLoop::new(Some("audio-manager"), None)? };
        let lock = thread_loop.lock();
        thread_loop.start();
        let context = pw::context::Context::new(&thread_loop)?;
        let core = context.connect(None)?;

        let thread_clone = thread_loop.clone();
        let pending = core.sync(0).expect("sync failed");
        let _listener_core = core
            .add_listener_local()
            .done(move |id, seq| {
                if id == PW_ID_CORE && seq == pending {
                    thread_clone.signal(false);
                }
            })
            .register();

        thread_loop.wait();
        lock.unlock();

        Ok(Self {
            muted: false,
            playback: None,
            thread_loop,
            context,
            core,
        })
    }

    pub fn play<T>(&mut self, path: T) -> anyhow::Result<()>
    where
        T: AsRef<Path>,
    {
        if self.muted {
            return Ok(());
        }

        let lock = self.thread_loop.lock();

        if let Some(playback) = self.playback.take() {
            playback.stop();
        }

        let playback = playback::Playback::new(self.thread_loop.clone(), &self.core, &path)?;

        lock.unlock();

        self.playback = Some(playback.start());
        Ok(())
    }

    pub fn mute(&mut self) {
        self.muted = true;
    }

    pub fn unmute(&mut self) {
        self.muted = false;
    }

    pub fn muted(&self) -> bool {
        self.muted
    }
}
