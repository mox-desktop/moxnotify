pub mod moxnotify {
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
}

pub mod color;
pub mod keymaps;

pub use moxnotify::types::Urgency;

use crate::types::LogLevel;
use keymaps::Keymaps;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Deserialize, Default, Clone)]
pub struct SoundFile {
    pub urgency_low: Option<Arc<Path>>,
    pub urgency_normal: Option<Arc<Path>>,
    pub urgency_critical: Option<Arc<Path>>,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct History {
    pub size: i64,
}

impl Default for History {
    fn default() -> Self {
        Self { size: 100 }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct General {
    pub history: History,
    pub theme: Option<Box<str>>,
    pub default_sound_file: SoundFile,
    pub ignore_sound_file: bool,
    pub scroll_sensitivity: f64,
    pub hint_characters: Box<str>,
    pub max_visible: usize,
    pub icon_size: u32,
    pub app_icon_size: u32,
    pub anchor: Anchor,
    pub layer: Layer,
    pub output: Option<Arc<str>>,
    pub ignore_timeout: bool,
    pub margin: Insets,
}

impl Default for General {
    fn default() -> Self {
        Self {
            default_sound_file: SoundFile::default(),
            ignore_sound_file: false,
            theme: None,
            hint_characters: "sadfjklewcmpgh".into(),
            scroll_sensitivity: 20.,
            max_visible: 5,
            icon_size: 64,
            app_icon_size: 24,
            anchor: Anchor::default(),
            layer: Layer::default(),
            output: None,
            ignore_timeout: false,
            history: History::default(),
            margin: Insets::default(),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ClientConfig {
    pub general: General,
    pub keymaps: Keymaps,
    pub css: String,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
}

fn default_log_level() -> LogLevel {
    LogLevel::default()
}

#[derive(Default, Clone, Copy, Deserialize, Debug)]
#[serde(default)]
pub struct Insets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Insets {
    pub fn size(value: f32) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }
}

impl From<Insets> for [f32; 4] {
    fn from(value: Insets) -> Self {
        [value.left, value.right, value.top, value.bottom]
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Background,
    Bottom,
    Top,
    #[default]
    Overlay,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    #[default]
    TopRight,
    TopCenter,
    TopLeft,
    BottomRight,
    BottomCenter,
    BottomLeft,
    CenterRight,
    CenterLeft,
    Center,
}

pub fn xdg_config_dir() -> anyhow::Result<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map_err(Into::into)
}

impl ClientConfig {
    pub fn load<T>(path: Option<T>) -> Self
    where
        T: AsRef<Path>,
    {
        let nix_code = if let Some(p) = path {
            match std::fs::read_to_string(p) {
                Ok(content) => content,
                Err(e) => {
                    log::error!("Failed to read config file: {e}");
                    return Self::default();
                }
            }
        } else {
            match xdg_config_dir() {
                Ok(base) => {
                    let candidates = [
                        base.join("mox/moxnotify/default.nix"),
                        base.join("mox/moxnotify.nix"),
                    ];
                    match candidates
                        .iter()
                        .find_map(|p| std::fs::read_to_string(p).ok())
                    {
                        Some(content) => content,
                        None => {
                            log::warn!("Config file not found");
                            return Self::default();
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to determine config directory: {e}");
                    return Self::default();
                }
            }
        };

        match tvix_serde::from_str(&nix_code) {
            Ok(config) => config,
            Err(e) => {
                log::error!("{e}");
                Self::default()
            }
        }
    }
}
