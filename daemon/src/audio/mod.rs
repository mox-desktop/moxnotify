mod playback;
pub mod tinyaudio;

use std::{collections::BTreeMap, path::Path};

#[derive(Default)]
struct Cache(BTreeMap<Box<Path>, playback::Playback>);

impl Cache {
    fn insert<P>(&mut self, icon_path: &P, data: playback::Playback)
    where
        P: AsRef<Path>,
    {
        let entry = icon_path.as_ref();
        self.0.insert(entry.into(), data);
    }

    fn get<P>(&self, icon_path: P) -> Option<playback::Playback>
    where
        P: AsRef<Path>,
    {
        self.0.get(icon_path.as_ref()).cloned()
    }
}

#[derive(Default)]
pub struct Audio {
    cache: Cache,
    muted: bool,
    playback: Option<playback::Playback<playback::Played>>,
}

impl Audio {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn play<T>(&mut self, path: T) -> anyhow::Result<()>
    where
        T: AsRef<Path>,
    {
        if self.muted {
            return Ok(());
        }

        if let Some(playback) = self.playback.take() {
            playback.stop();
        }

        let playback = match self.cache.get(&path) {
            Some(playback) => playback,
            None => {
                let playback = playback::Playback::new(&path).unwrap();
                self.cache.insert(&path, playback.clone());
                playback
            }
        };

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
