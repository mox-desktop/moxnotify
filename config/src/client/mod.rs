pub mod moxnotify {
    pub mod types {
        tonic::include_proto!("moxnotify.types");
    }
}

pub mod border;
pub mod button;
pub mod color;
pub mod keymaps;
pub mod partial;
pub mod text;

pub use moxnotify::types::Urgency;

use crate::types::LogLevel;
use border::{Border, BorderRadius};
use button::{Button, ButtonState, Buttons};
use color::Color;
use keymaps::Keymaps;
use partial::{PartialFont, PartialInsets, PartialStyle};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use text::{Body, Summary};

#[derive(Default, Clone)]
pub struct SoundFile {
    pub urgency_low: Option<Arc<Path>>,
    pub urgency_normal: Option<Arc<Path>>,
    pub urgency_critical: Option<Arc<Path>>,
}

impl<'de> Deserialize<'de> for SoundFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SoundFileVisitor;

        impl<'de> serde::de::Visitor<'de> for SoundFileVisitor {
            type Value = SoundFile;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or a map")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SoundFile {
                    urgency_low: Some(Path::new(v).into()),
                    urgency_normal: Some(Path::new(v).into()),
                    urgency_critical: Some(Path::new(v).into()),
                })
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SoundFile {
                    urgency_low: Some(Path::new(&v).into()),
                    urgency_normal: Some(Path::new(&v).into()),
                    urgency_critical: Some(Path::new(&v).into()),
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut urgency_low = None;
                let mut urgency_normal = None;
                let mut urgency_critical = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "urgency_low" => urgency_low = Some(map.next_value()?),
                        "urgency_normal" => urgency_normal = Some(map.next_value()?),
                        "urgency_critical" => urgency_critical = Some(map.next_value()?),
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &["urgency_low", "urgency_normal", "urgency_critical"],
                            ));
                        }
                    }
                }

                Ok(SoundFile {
                    urgency_low,
                    urgency_normal,
                    urgency_critical,
                })
            }
        }

        deserializer.deserialize_any(SoundFileVisitor)
    }
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
            theme: None,
            default_sound_file: SoundFile::default(),
            ignore_sound_file: false,
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
    pub styles: Styles,
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

    fn apply(&mut self, partial: &PartialInsets) {
        if let Some(left) = partial.left {
            self.left = left;
        }
        if let Some(right) = partial.right {
            self.right = right;
        }
        if let Some(top) = partial.top {
            self.top = top;
        }
        if let Some(bottom) = partial.bottom {
            self.bottom = bottom;
        }
    }
}

impl From<Insets> for [f32; 4] {
    fn from(value: Insets) -> Self {
        [value.left, value.right, value.top, value.bottom]
    }
}

#[derive(Clone)]
pub struct Font {
    pub size: f32,
    pub family: Arc<str>,
    pub color: Color,
}

impl Font {
    fn apply(&mut self, partial: &PartialFont) {
        if let Some(size) = partial.size {
            self.size = size;
        }
        if let Some(family) = partial.family.as_ref().map(Arc::clone) {
            self.family = family;
        }
        if let Some(color) = partial.color.as_ref() {
            self.color.apply(color);
        }
    }
}

impl Default for Font {
    fn default() -> Self {
        Self {
            size: 10.,
            family: "DejaVu Sans".into(),
            color: Color::rgba([255, 255, 255, 255]),
        }
    }
}

#[derive(Clone)]
pub struct Icon {
    pub border: Border,
}

impl Icon {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
    }
}

impl Default for Icon {
    fn default() -> Self {
        Self {
            border: Border {
                color: Color::default(),
                size: Insets::size(0.),
                radius: BorderRadius::default(),
            },
        }
    }
}

#[derive(Clone)]
pub struct Progress {
    pub border: Border,
    pub incomplete_color: Color,
    pub complete_color: Color,
}

impl Progress {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.complete_color.apply(background);
        }
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            border: Border {
                radius: BorderRadius {
                    top_left: 5.,
                    top_right: 5.,
                    bottom_left: 5.,
                    bottom_right: 5.,
                },
                ..Default::default()
            },
            incomplete_color: Color::default(),
            complete_color: Color {
                urgency_low: [242, 205, 205, 255],
                urgency_normal: [242, 205, 205, 255],
                urgency_critical: [243, 139, 168, 255],
            },
        }
    }
}

#[derive(Clone)]
pub struct Hint {
    pub background: Color,
    pub font: Font,
    pub border: Border,
}

impl Hint {
    fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.background.apply(background);
        }
        if let Some(font) = partial.font.as_ref() {
            self.font.apply(font);
        }
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
    }
}

impl Default for Hint {
    fn default() -> Self {
        Self {
            background: Color {
                urgency_low: [30, 30, 46, 255],
                urgency_normal: [24, 24, 37, 255],
                urgency_critical: [24, 24, 37, 255],
            },
            font: Font::default(),
            border: Border::default(),
        }
    }
}

#[derive(Clone)]
pub struct StyleState {
    pub hint: Hint,
    pub background: Color,
    pub font: Font,
    pub border: Border,
    pub icon: Icon,
    pub app_icon: Icon,
    pub progress: Progress,
    pub buttons: Buttons,
    pub summary: Summary,
    pub body: Body,
}

impl StyleState {
    fn default_hover() -> Self {
        Self {
            background: Color::rgba([47, 53, 73, 255]),
            ..Default::default()
        }
    }

    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.background.apply(background);
        }
        if let Some(partial_font) = partial.font.as_ref() {
            self.font.apply(partial_font);
        }
        if let Some(partial_border) = partial.border.as_ref() {
            self.border.apply(partial_border);
        }
    }
}

impl Default for StyleState {
    fn default() -> Self {
        Self {
            body: Body::default(),
            summary: Summary::default(),
            hint: Hint::default(),
            background: Color {
                urgency_low: [26, 27, 38, 255],
                urgency_normal: [22, 22, 30, 255],
                urgency_critical: [22, 22, 30, 255],
            },
            font: Font::default(),
            border: Border::default(),
            icon: Icon::default(),
            app_icon: Icon::default(),
            progress: Progress::default(),
            buttons: Buttons::default(),
        }
    }
}

#[derive(Deserialize, Default)]
pub struct Styles {
    pub urgency_low: UrgencyStyles,
    pub urgency_normal: UrgencyStyles,
    pub urgency_critical: UrgencyStyles,
    pub next: NotificationCounter,
    pub prev: NotificationCounter,
}

pub struct UrgencyStyles {
    pub focused: StyleState,
    pub unfocused: StyleState,
}

impl<'de> Deserialize<'de> for UrgencyStyles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct UrgencyStylesHelper {
            #[serde(default)]
            focused: Option<PartialStyle>,
            #[serde(default)]
            unfocused: Option<PartialStyle>,
        }

        let helper = UrgencyStylesHelper::deserialize(deserializer)?;
        let mut focused = StyleState::default_hover();
        let mut unfocused = StyleState {
            buttons: Buttons {
                dismiss: Button {
                    default: ButtonState {
                        background: Color::rgba([0, 0, 0, 0]),
                        border: Border {
                            size: Insets {
                                left: 0.,
                                right: 0.,
                                top: 0.,
                                bottom: 0.,
                            },
                            radius: BorderRadius::circle(),
                            color: Color::rgba([0, 0, 0, 0]),
                        },
                        font: Font {
                            color: Color::rgba([0, 0, 0, 0]),
                            ..Default::default()
                        },
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        if let Some(partial) = helper.focused {
            focused.apply(&partial);
            focused.progress.apply(&partial);
            focused.icon.apply(&partial);
            focused.app_icon.apply(&partial);
            focused.buttons.action.apply(&partial);
            focused.buttons.dismiss.apply(&partial);
            focused.hint.apply(&partial);
            focused.summary.apply(&partial);
            focused.body.apply(&partial);
        }
        if let Some(partial) = helper.unfocused {
            unfocused.apply(&partial);
            unfocused.progress.apply(&partial);
            unfocused.icon.apply(&partial);
            unfocused.app_icon.apply(&partial);
            unfocused.buttons.action.apply(&partial);
            unfocused.buttons.dismiss.apply(&partial);
            unfocused.hint.apply(&partial);
            unfocused.summary.apply(&partial);
            unfocused.body.apply(&partial);
        }

        Ok(UrgencyStyles { focused, unfocused })
    }
}

impl Default for UrgencyStyles {
    fn default() -> Self {
        Self {
            focused: StyleState::default_hover(),
            unfocused: StyleState {
                buttons: Buttons {
                    dismiss: Button {
                        default: ButtonState {
                            background: Color::rgba([0, 0, 0, 0]),
                            border: Border {
                                size: Insets {
                                    left: 0.,
                                    right: 0.,
                                    top: 0.,
                                    bottom: 0.,
                                },
                                radius: BorderRadius::circle(),
                                color: Color::rgba([0, 0, 0, 0]),
                            },
                            font: Font {
                                color: Color::rgba([0, 0, 0, 0]),
                                ..Default::default()
                            },
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        }
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

pub struct NotificationCounter {
    pub format: Box<str>,
    pub border: Border,
    pub background: Color,
    pub font: Font,
}

impl<'de> Deserialize<'de> for NotificationCounter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NotificationCounterHelper {
            #[serde(default)]
            format: Option<Box<str>>,
            #[serde(default)]
            style: Option<PartialStyle>,
        }

        let helper = NotificationCounterHelper::deserialize(deserializer)?;
        let mut counter = NotificationCounter {
            format: helper.format.unwrap_or_else(default_counter_format),
            border: Border::default(),
            background: default_counter_background(),
            font: Font::default(),
        };

        if let Some(partial) = helper.style {
            counter.apply(&partial);
        }

        Ok(counter)
    }
}

impl Default for NotificationCounter {
    fn default() -> Self {
        Self {
            format: default_counter_format(),
            border: Border::default(),
            background: default_counter_background(),
            font: Font::default(),
        }
    }
}

fn default_counter_format() -> Box<str> {
    "({} more)".into()
}

fn default_counter_background() -> Color {
    Color::rgba([26, 27, 38, 255])
}

impl NotificationCounter {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.background.apply(background);
        }
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
    }
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

    pub fn find_style(&self, urgency: Urgency, focused: bool) -> &StyleState {
        let urgency_styles = match urgency {
            Urgency::Low => &self.styles.urgency_low,
            Urgency::Normal => &self.styles.urgency_normal,
            Urgency::Critical => &self.styles.urgency_critical,
        };
        if focused {
            &urgency_styles.focused
        } else {
            &urgency_styles.unfocused
        }
    }

    pub fn path() -> anyhow::Result<Box<Path>> {
        let home_dir = std::env::var("HOME").map(PathBuf::from)?;
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map_or_else(|_| home_dir.join(".config"), PathBuf::from);

        let mox_path = config_dir.join("mox").join("moxnotify").join("config.lua");
        if mox_path.exists() {
            return Ok(mox_path.into());
        }

        let standard_path = config_dir.join("moxnotify").join("config.lua");

        Ok(standard_path.into())
    }
}
