pub mod border;
pub mod button;
pub mod color;
pub mod keymaps;
pub mod partial;
pub mod text;

use border::{Border, BorderRadius};
use button::{Button, ButtonState, Buttons};
use color::Color;
use keymaps::Keymaps;
use mlua::{Lua, LuaSerdeExt};
use partial::{PartialFont, PartialInsets, PartialStyle};
use serde::{Deserialize, Deserializer};
use std::{
    fmt, fs,
    ops::{Add, Sub},
    path::{Path, PathBuf},
    sync::Arc,
};
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
    pub path: PathBuf,
}

impl Default for History {
    fn default() -> Self {
        let path = std::env::var("XDG_DATA_HOME")
            .map(|data_home| PathBuf::from(data_home).join("moxnotify/db.mox"))
            .or_else(|_| {
                std::env::var("HOME")
                    .map(|home| PathBuf::from(home).join(".local/share/moxnotify/db.mox"))
            })
            .unwrap_or_else(|_| PathBuf::from(""));

        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }

        Self { size: 100, path }
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
    pub margin: Insets,
    pub anchor: Anchor,
    pub layer: Layer,
    pub queue: Queue,
    pub output: Option<Box<str>>,
    pub default_timeout: Timeout,
    pub ignore_timeout: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            theme: None,
            margin: Insets::default(),
            history: History::default(),
            default_sound_file: SoundFile::default(),
            ignore_sound_file: false,
            hint_characters: "sadfjklewcmpgh".into(),
            scroll_sensitivity: 20.,
            max_visible: 5,
            icon_size: 64,
            app_icon_size: 24,
            anchor: Anchor::default(),
            layer: Layer::default(),
            queue: Queue::default(),
            output: None,
            default_timeout: Timeout::default(),
            ignore_timeout: false,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub styles: Styles,
    pub keymaps: Keymaps,
}

#[derive(Deserialize)]
pub struct Style {
    #[serde(deserialize_with = "deserialize_selectors")]
    pub selector: Vec<Selector>,
    #[serde(default)]
    pub state: State,
    pub style: PartialStyle,
    #[serde(default)]
    pub default_timeout: Option<Timeout>,
    #[serde(default)]
    pub ignore_timeout: Option<bool>,
    #[serde(default)]
    pub default_sound_file: Option<SoundFile>,
    #[serde(default)]
    pub ignore_sound_file: Option<bool>,
    #[serde(default)]
    pub theme: Option<Arc<str>>,
}

fn deserialize_selectors<'de, D>(deserializer: D) -> Result<Vec<Selector>, D::Error>
where
    D: Deserializer<'de>,
{
    struct SelectorsVisitor;

    impl<'de> serde::de::Visitor<'de> for SelectorsVisitor {
        type Value = Vec<Selector>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or a list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let selector = Selector::deserialize(serde::de::value::StrDeserializer::new(value))?;
            Ok(vec![selector])
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut selectors = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                let selector = Selector::deserialize(serde::de::value::StrDeserializer::new(&s))?;
                selectors.push(selector);
            }
            Ok(selectors)
        }
    }

    deserializer.deserialize_any(SelectorsVisitor)
}

#[derive(Default, Clone)]
pub enum State {
    #[default]
    Default,
    Hover,
    ContainerHover,
    NamedContainerHover(Arc<str>),
}

impl<'de> Deserialize<'de> for State {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "default" => Ok(State::Default),
            "hover" => Ok(State::Hover),
            "container_hover" => Ok(State::ContainerHover),
            _ => {
                if let Some(container) = s.strip_prefix("container_hover:") {
                    Ok(State::NamedContainerHover(container.into()))
                } else {
                    Err(serde::de::Error::unknown_variant(
                        &s,
                        &["default", "hover", "container_hover", "container_hover:..."],
                    ))
                }
            }
        }
    }
}

pub enum Selector {
    All,
    PrevCounter,
    NextCounter,
    AllNotifications,
    Notification(Box<str>),
    ActionButton,
    DismissButton,
    Progress,
    Icon,
    Hints,
    Summary,
    Body,
}

impl<'de> Deserialize<'de> for Selector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "*" => Ok(Selector::All),
            "prev_counter" => Ok(Selector::PrevCounter),
            "next_counter" => Ok(Selector::NextCounter),
            "notification" => Ok(Selector::AllNotifications),
            "action" => Ok(Selector::ActionButton),
            "dismiss" => Ok(Selector::DismissButton),
            "progress" => Ok(Selector::Progress),
            "icon" => Ok(Selector::Icon),
            "hints" => Ok(Selector::Hints),
            "summary" => Ok(Selector::Summary),
            "body" => Ok(Selector::Body),
            _ => {
                if let Some(notification) = s.strip_prefix("notification:") {
                    Ok(Selector::Notification(notification.into()))
                } else {
                    Err(serde::de::Error::unknown_variant(
                        &s,
                        &[
                            "*",
                            "prev_counter",
                            "next_counter",
                            "notification",
                            "notification:...",
                            "action",
                            "dismiss",
                        ],
                    ))
                }
            }
        }
    }
}

#[derive(Default, Clone, Copy, Deserialize, Debug)]
#[serde(default)]
pub struct Insets {
    pub left: Size,
    pub right: Size,
    pub top: Size,
    pub bottom: Size,
}

impl Insets {
    pub fn size(value: Size) -> Self {
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
        [
            value.left.resolve(0.),
            value.right.resolve(0.),
            value.top.resolve(0.),
            value.bottom.resolve(0.),
        ]
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

#[derive(Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::upper_case_acronyms)]
pub enum Queue {
    #[default]
    Unordered,
    FIFO,
}

#[derive(Clone)]
pub struct Icon {
    pub border: Border,
    pub margin: Insets,
    pub padding: Insets,
}

impl Icon {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
        if let Some(margin) = partial.margin.as_ref() {
            self.margin.apply(margin);
        }
        if let Some(padding) = partial.padding.as_ref() {
            self.padding.apply(padding);
        }
    }
}

impl Default for Icon {
    fn default() -> Self {
        Self {
            border: Border {
                color: Color::default(),
                size: Insets::size(Size::Value(0.)),
                radius: BorderRadius::default(),
            },
            margin: Insets {
                right: Size::Value(10.),
                left: Size::Value(5.),
                ..Default::default()
            },
            padding: Insets::default(),
        }
    }
}

#[derive(Deserialize, Default, Debug, Clone, Copy, PartialEq)]
pub enum Size {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    Value(f32),
}

impl Add for Size {
    type Output = f32;

    fn add(self, rhs: Self) -> Self::Output {
        self.resolve(0.) + rhs.resolve(0.)
    }
}

impl Add<f32> for Size {
    type Output = f32;

    fn add(self, rhs: f32) -> Self::Output {
        self.resolve(0.) + rhs
    }
}

impl Add<Size> for f32 {
    type Output = f32;

    fn add(self, rhs: Size) -> Self::Output {
        self + rhs.resolve(0.)
    }
}

impl Sub for Size {
    type Output = f32;

    fn sub(self, rhs: Self) -> Self::Output {
        self.resolve(0.) - rhs.resolve(0.)
    }
}

impl Sub<f32> for Size {
    type Output = f32;

    fn sub(self, rhs: f32) -> Self::Output {
        self.resolve(0.) - rhs
    }
}

impl Sub<Size> for f32 {
    type Output = f32;

    fn sub(self, rhs: Size) -> Self::Output {
        self - rhs.resolve(0.)
    }
}

impl Size {
    pub fn is_auto(self) -> bool {
        self == Size::Auto
    }

    pub fn is_value(self) -> bool {
        matches!(self, Size::Auto)
    }

    pub fn resolve(self, auto: f32) -> f32 {
        match self {
            Size::Auto => auto,
            Size::Value(v) => v,
        }
    }
}

#[derive(Clone)]
pub struct Progress {
    pub margin: Insets,
    pub height: Size,
    pub width: Size,
    pub border: Border,
    pub incomplete_color: Color,
    pub complete_color: Color,
}

impl Progress {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.complete_color.apply(background);
        }
        if let Some(margin) = partial.margin.as_ref() {
            self.margin.apply(margin);
        }
        if let Some(height) = partial.height.as_ref() {
            self.height = *height;
        }
        if let Some(width) = partial.width.as_ref() {
            self.width = *width;
        }
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            margin: Insets {
                left: Size::Auto,
                right: Size::Auto,
                top: Size::Value(10.),
                bottom: Size::Value(0.),
            },
            height: Size::Value(20.),
            width: Size::Auto,
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
    pub width: Size,
    pub height: Size,
    pub font: Font,
    pub border: Border,
    pub padding: Insets,
    pub margin: Insets,
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
        if let Some(margin) = partial.margin.as_ref() {
            self.margin.apply(margin);
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
            width: Size::Value(15.),
            height: Size::Value(20.),
            font: Font::default(),
            border: Border::default(),
            padding: Insets {
                left: Size::Auto,
                right: Size::Auto,
                top: Size::Auto,
                bottom: Size::Auto,
            },
            margin: Insets::default(),
        }
    }
}

#[derive(Clone)]
pub struct StyleState {
    pub hint: Hint,
    pub background: Color,
    pub width: Size,
    pub min_height: Size,
    pub max_height: Size,
    pub height: Size,
    pub font: Font,
    pub border: Border,
    pub margin: Insets,
    pub padding: Insets,
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
        if let Some(width) = partial.width {
            self.width = width;
        }
        if let Some(min_height) = partial.min_height {
            self.min_height = min_height;
        }
        if let Some(max_height) = partial.max_height {
            self.max_height = max_height;
        }
        if let Some(height) = partial.height {
            self.height = height;
        }
        if let Some(partial_font) = partial.font.as_ref() {
            self.font.apply(partial_font);
        }
        if let Some(partial_border) = partial.border.as_ref() {
            self.border.apply(partial_border);
        }
        if let Some(partial_margin) = partial.margin.as_ref() {
            self.margin.apply(partial_margin);
        }
        if let Some(partial_padding) = partial.padding.as_ref() {
            self.padding.apply(partial_padding);
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
            width: Size::Value(300.),
            min_height: Size::Auto,
            max_height: Size::Auto,
            height: Size::Auto,
            font: Font::default(),
            border: Border::default(),
            margin: Insets::size(Size::Value(5.)),
            padding: Insets::size(Size::Value(10.)),
            icon: Icon::default(),
            app_icon: Icon::default(),
            progress: Progress::default(),
            buttons: Buttons::default(),
        }
    }
}

pub struct Styles {
    pub next: NotificationCounter,
    pub prev: NotificationCounter,
    pub notification: Vec<NotificationStyleEntry>,
    pub default: StyleState,
    pub hover: StyleState,
}

impl<'de> Deserialize<'de> for Styles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct TempStyles(Vec<Style>);

        impl TempStyles {
            fn priority(style: &Style) -> u8 {
                match (&style.selector[0], &style.state) {
                    (Selector::All, _) => 1,
                    (Selector::AllNotifications, State::Default) => 2,
                    (Selector::AllNotifications, State::Hover) => 3,
                    (Selector::AllNotifications, State::ContainerHover) => 4,
                    (Selector::AllNotifications, State::NamedContainerHover(_)) => 5,
                    (Selector::Notification(_), State::Default) => 6,
                    (Selector::Notification(_), State::Hover) => 7,
                    (Selector::Notification(_), State::ContainerHover) => 8,
                    (Selector::Notification(_), State::NamedContainerHover(_)) => 9,
                    (Selector::ActionButton, State::Default) => 10,
                    (Selector::ActionButton, State::Hover) => 11,
                    (Selector::ActionButton, State::ContainerHover) => 12,
                    (Selector::ActionButton, State::NamedContainerHover(_)) => 13,
                    (Selector::DismissButton, State::Default) => 14,
                    (Selector::DismissButton, State::Hover) => 15,
                    (Selector::DismissButton, State::ContainerHover) => 16,
                    (Selector::DismissButton, State::NamedContainerHover(_)) => 17,
                    (Selector::Icon, _) => 18,
                    (Selector::Progress, _) => 19,
                    (Selector::PrevCounter, _) => 20,
                    (Selector::NextCounter, _) => 21,
                    (Selector::Hints, _) => 22,
                    (Selector::Summary, _) => 23,
                    (Selector::Body, _) => 24,
                }
            }

            fn sort(mut self) -> Self {
                self.0.sort_by_key(Self::priority);
                self
            }
        }

        let mut styles = Styles::default();

        let temp_styles = {
            let styles_vec = TempStyles::deserialize(deserializer)?
                .0
                .into_iter()
                .flat_map(|style| {
                    style.selector.into_iter().map(move |selector| Style {
                        selector: vec![selector],
                        state: style.state.clone(),
                        style: style.style.clone(),
                        default_timeout: style.default_timeout,
                        ignore_timeout: style.ignore_timeout,
                        default_sound_file: style.default_sound_file.clone(),
                        ignore_sound_file: style.ignore_sound_file,
                        theme: style.theme.clone(),
                    })
                })
                .collect::<Vec<_>>();
            TempStyles(styles_vec).sort()
        };

        temp_styles.0.iter().for_each(|style| {
            match (style.selector.first().unwrap(), &style.state) {
                (Selector::All, _) => {
                    styles.default.apply(&style.style);
                    styles.hover.apply(&style.style);

                    styles.prev.apply(&style.style);
                    styles.next.apply(&style.style);

                    styles.default.progress.apply(&style.style);
                    styles.hover.progress.apply(&style.style);

                    styles.default.icon.apply(&style.style);
                    styles.hover.icon.apply(&style.style);

                    styles.default.buttons.action.apply(&style.style);
                    styles.hover.buttons.action.apply(&style.style);

                    styles.default.buttons.dismiss.apply(&style.style);
                    styles.hover.buttons.dismiss.apply(&style.style);

                    styles.default.hint.apply(&style.style);
                    styles.hover.hint.apply(&style.style);

                    styles.default.summary.apply(&style.style);
                    styles.hover.summary.apply(&style.style);

                    styles.default.body.apply(&style.style);
                    styles.hover.body.apply(&style.style);
                }
                (Selector::Hints, _) => {
                    styles.default.hint.apply(&style.style);
                    styles.hover.hint.apply(&style.style);
                }
                (Selector::NextCounter, _) => styles.next.apply(&style.style),
                (Selector::PrevCounter, _) => styles.prev.apply(&style.style),
                (Selector::Summary, State::ContainerHover) => {
                    styles.default.summary.apply(&style.style);
                    styles.hover.summary.apply(&style.style);
                }
                (Selector::Summary, State::NamedContainerHover(app_name)) => {
                    if let Some(notification) = styles
                        .notification
                        .iter_mut()
                        .find(|notification| *notification.app == **app_name)
                    {
                        notification.hover.summary.apply(&style.style);
                    } else {
                        let mut notification = NotificationStyleEntry {
                            default: styles.default.clone(),
                            hover: styles.hover.clone(),
                            app: (&**app_name).into(),
                            ..Default::default()
                        };
                        notification.hover.summary.apply(&style.style);
                        styles.notification.push(notification);
                    }
                }
                (Selector::Summary, _) => {
                    styles.default.summary.apply(&style.style);
                    styles.hover.summary.apply(&style.style);
                }
                (Selector::Body, State::ContainerHover) => {
                    styles.default.body.apply(&style.style);
                    styles.hover.body.apply(&style.style);
                }
                (Selector::Body, State::NamedContainerHover(app_name)) => {
                    if let Some(notification) = styles
                        .notification
                        .iter_mut()
                        .find(|notification| *notification.app == **app_name)
                    {
                        notification.hover.body.apply(&style.style);
                    } else {
                        let mut notification = NotificationStyleEntry {
                            default: styles.default.clone(),
                            hover: styles.hover.clone(),
                            app: (&**app_name).into(),
                            ..Default::default()
                        };
                        notification.hover.body.apply(&style.style);
                        styles.notification.push(notification);
                    }
                }
                (Selector::Body, _) => {
                    styles.default.body.apply(&style.style);
                    styles.hover.body.apply(&style.style);
                }
                (Selector::Progress, State::ContainerHover) => {
                    styles.hover.progress.apply(&style.style);
                }
                (Selector::Progress, State::NamedContainerHover(app_name)) => {
                    if let Some(notification) = styles
                        .notification
                        .iter_mut()
                        .find(|notification| *notification.app == **app_name)
                    {
                        notification.hover.progress.apply(&style.style);
                    } else {
                        let mut notification = NotificationStyleEntry {
                            default: styles.default.clone(),
                            hover: styles.hover.clone(),
                            app: (&**app_name).into(),
                            ..Default::default()
                        };
                        notification.hover.progress.apply(&style.style);
                        styles.notification.push(notification);
                    }
                }
                (Selector::Progress, _) => {
                    styles.default.progress.apply(&style.style);
                    styles.hover.progress.apply(&style.style);
                }
                (Selector::Icon, State::ContainerHover) => {
                    styles.default.icon.apply(&style.style);
                    styles.hover.icon.apply(&style.style);
                }
                (Selector::Icon, State::NamedContainerHover(app_name)) => {
                    if let Some(notification) = styles
                        .notification
                        .iter_mut()
                        .find(|notification| *notification.app == **app_name)
                    {
                        notification.hover.icon.apply(&style.style);
                    } else {
                        let mut notification = NotificationStyleEntry {
                            default: styles.default.clone(),
                            hover: styles.hover.clone(),
                            app: (&**app_name).into(),
                            ..Default::default()
                        };
                        notification.hover.icon.apply(&style.style);
                        styles.notification.push(notification);
                    }
                }
                (Selector::Icon, _) => {
                    styles.default.icon.apply(&style.style);
                    styles.hover.icon.apply(&style.style);
                }

                (Selector::AllNotifications, State::Default) => {
                    styles.default.apply(&style.style);
                    styles.hover.apply(&style.style);
                }
                (Selector::AllNotifications, _) => {
                    styles.hover.apply(&style.style);
                }
                (Selector::Notification(app_name), State::Default) => {
                    if let Some(notification) = styles
                        .notification
                        .iter_mut()
                        .find(|notification| *notification.app == **app_name)
                    {
                        notification.default.apply(&style.style);
                    } else {
                        let mut notification = NotificationStyleEntry {
                            default: styles.default.clone(),
                            hover: styles.hover.clone(),
                            app: (&**app_name).into(),
                            ..Default::default()
                        };
                        notification.default.apply(&style.style);
                        styles.notification.push(notification);
                    }
                }
                (Selector::Notification(app_name), _) => {
                    if let Some(notification) = styles
                        .notification
                        .iter_mut()
                        .find(|notification| *notification.app == **app_name)
                    {
                        notification.hover.apply(&style.style);
                    } else {
                        let mut notification = NotificationStyleEntry {
                            default: styles.default.clone(),
                            hover: styles.hover.clone(),
                            app: (&**app_name).into(),
                            ..Default::default()
                        };
                        notification.hover.apply(&style.style);
                        styles.notification.push(notification);
                    }
                }
                (Selector::ActionButton, State::Default) => {
                    styles.default.buttons.action.apply(&style.style);
                    styles.hover.buttons.action.apply(&style.style);
                }
                (Selector::ActionButton, State::Hover) => {
                    styles.default.buttons.action.apply_hover(&style.style);
                    styles.hover.buttons.action.apply_hover(&style.style);
                }
                (Selector::ActionButton, State::ContainerHover) => {
                    styles.hover.buttons.action.apply(&style.style);
                }
                (Selector::ActionButton, State::NamedContainerHover(app_name)) => {
                    if let Some(notification) = styles
                        .notification
                        .iter_mut()
                        .find(|notification| *notification.app == **app_name)
                    {
                        notification.hover.buttons.action.apply(&style.style);
                    } else {
                        let mut notification = NotificationStyleEntry {
                            default: styles.default.clone(),
                            hover: styles.hover.clone(),
                            app: (&**app_name).into(),
                            ..Default::default()
                        };
                        notification.hover.buttons.action.apply(&style.style);
                        styles.notification.push(notification);
                    }
                }
                (Selector::DismissButton, State::Default) => {
                    styles.default.buttons.dismiss.apply(&style.style);
                    styles.hover.buttons.dismiss.apply(&style.style);
                }
                (Selector::DismissButton, State::Hover) => {
                    styles.default.buttons.dismiss.apply_hover(&style.style);
                    styles.hover.buttons.dismiss.apply_hover(&style.style);
                }
                (Selector::DismissButton, State::ContainerHover) => {
                    styles.hover.buttons.dismiss.apply(&style.style);
                }
                (Selector::DismissButton, State::NamedContainerHover(app_name)) => {
                    if let Some(notification) = styles
                        .notification
                        .iter_mut()
                        .find(|notification| *notification.app == **app_name)
                    {
                        notification.hover.buttons.dismiss.apply(&style.style);
                    } else {
                        let mut notification = NotificationStyleEntry {
                            default: styles.default.clone(),
                            hover: styles.hover.clone(),
                            app: (&**app_name).into(),
                            ..Default::default()
                        };
                        notification.hover.buttons.dismiss.apply(&style.style);
                        styles.notification.push(notification);
                    }
                }
            }
        });

        Ok(styles)
    }
}

impl Default for Styles {
    fn default() -> Self {
        Self {
            next: NotificationCounter::default(),
            prev: NotificationCounter::default(),
            notification: Vec::new(),
            default: StyleState {
                buttons: Buttons {
                    dismiss: Button {
                        default: ButtonState {
                            background: Color::rgba([0, 0, 0, 0]),
                            border: Border {
                                size: Insets {
                                    left: Size::Value(0.),
                                    right: Size::Value(0.),
                                    top: Size::Value(0.),
                                    bottom: Size::Value(0.),
                                },
                                radius: BorderRadius::circle(),
                                color: Color::rgba([0, 0, 0, 0]),
                            },
                            font: Font {
                                color: Color::rgba([0, 0, 0, 0]),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            hover: StyleState::default_hover(),
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

#[derive(Clone, Copy)]
pub struct Timeout {
    urgency_low: i32,
    urgency_normal: i32,
    urgency_critical: i32,
}

impl Default for Timeout {
    fn default() -> Self {
        Self {
            urgency_low: 5,
            urgency_normal: 10,
            urgency_critical: 0,
        }
    }
}

impl Timeout {
    pub fn get(&self, urgency: crate::Urgency) -> i32 {
        match urgency {
            crate::Urgency::Low => self.urgency_low,
            crate::Urgency::Normal => self.urgency_normal,
            crate::Urgency::Critical => self.urgency_critical,
        }
    }
}

impl<'de> Deserialize<'de> for Timeout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TimeoutVisitor;

        impl<'de> serde::de::Visitor<'de> for TimeoutVisitor {
            type Value = Timeout;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or a map")
            }

            fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E> {
                let value = v as i32;
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
                let value = v as i32;
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E> {
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                let value = v as i32;
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E> {
                let value = v as i32;
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                let value = v as i32;
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
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

                Ok(Timeout {
                    urgency_low: urgency_low.unwrap_or_default(),
                    urgency_normal: urgency_normal.unwrap_or_default(),
                    urgency_critical: urgency_critical.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_any(TimeoutVisitor)
    }
}

#[derive(Default)]
pub struct NotificationStyleEntry {
    pub app: Arc<str>,
    pub default: StyleState,
    pub hover: StyleState,
    pub default_timeout: Option<Timeout>,
    pub ignore_timeout: Option<bool>,
    pub default_sound_file: Option<SoundFile>,
    pub ignore_sound_file: Option<bool>,
}

pub struct NotificationCounter {
    pub format: Box<str>,
    pub border: Border,
    pub background: Color,
    pub margin: Insets,
    pub padding: Insets,
    pub font: Font,
}

impl NotificationCounter {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.background.apply(background);
        }
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
        if let Some(margin) = partial.margin.as_ref() {
            self.margin.apply(margin);
        }
        if let Some(padding) = partial.padding.as_ref() {
            self.padding.apply(padding);
        }
    }
}

impl Default for NotificationCounter {
    fn default() -> Self {
        Self {
            format: "({} more)".into(),
            border: Border::default(),
            background: Color::rgba([26, 27, 38, 255]),
            margin: Insets::default(),
            padding: Insets::default(),
            font: Font::default(),
        }
    }
}

impl Config {
    pub fn load<T>(path: Option<T>) -> anyhow::Result<Self>
    where
        T: AsRef<Path>,
    {
        let lua_code = if let Some(path) = path {
            if !path.as_ref().exists() {
                return Ok(Self::default());
            }
            fs::read_to_string(path).unwrap_or_default()
        } else {
            let path = Self::path()?;
            if !path.exists() {
                return Ok(Self::default());
            }

            fs::read_to_string(path).unwrap_or_default()
        };

        let lua = Lua::new();

        let lua_result = lua
            .load(lua_code)
            .eval()
            .map_err(|e| anyhow::anyhow!("Lua evaluation error: {}", e))?;

        lua.from_value(lua_result)
            .map_err(|e| anyhow::anyhow!("Config deserialization error: {}", e))
    }

    pub fn find_style<T>(&self, app_name: T, hovered: bool) -> &StyleState
    where
        T: AsRef<str>,
    {
        self.styles
            .notification
            .iter()
            .find(|n| &*n.app == app_name.as_ref())
            .map_or_else(
                || {
                    if hovered {
                        &self.styles.hover
                    } else {
                        &self.styles.default
                    }
                },
                |c| {
                    if hovered { &c.hover } else { &c.default }
                },
            )
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
