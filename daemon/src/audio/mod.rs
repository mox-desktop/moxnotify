mod playback;

use pipewire::{self as pw, sys::PW_ID_CORE};
use std::path::Path;

pub struct Audio {
    muted: bool,
    playback: Option<playback::Playback<playback::Played>>,
    thread_loop: pw::thread_loop::ThreadLoopRc,
    _context: pw::context::ContextRc,
    core: pw::core::CoreRc,
}

impl Audio {
    pub fn try_new() -> anyhow::Result<Self> {
        pw::init();
        let thread_loop =
            unsafe { pw::thread_loop::ThreadLoopRc::new(Some("audio-manager"), None)? };
        let lock = thread_loop.lock();
        thread_loop.start();
        let context = pw::context::ContextRc::new(&thread_loop, None)?;
        let core = context.connect_rc(None)?;

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
            _context: context,
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

        if let Some(playback) = self.playback.take() {
            if let Some(cooldown) = playback.cooldown.as_ref()
                && cooldown.elapsed() > std::time::Duration::from_millis(20)
            {
                let lock = self.thread_loop.lock();
                playback.stop();
                lock.unlock();
            } else {
                self.playback = Some(playback);
                return Ok(());
            }
        }

        let playback = playback::Playback::new(self.thread_loop.clone(), self.core.clone(), &path)?;
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
