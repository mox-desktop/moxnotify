//! Minimal local styling types with hardcoded defaults.
//! This module exists as a bridge until CSS styling (simplecss) is implemented.

pub use config::client::color::Color;
use config::client::Urgency;
use std::sync::Arc;

#[derive(Clone)]
pub struct Font {
    pub size: u32,
    pub family: Arc<str>,
    pub color: Color,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            size: 10,
            family: "DejaVu Sans".into(),
            color: Color::rgba([255, 255, 255, 255]),
        }
    }
}

#[derive(Default, Clone, Copy)]
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

#[derive(Default, Clone, Copy)]
pub struct BorderRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_left: f32,
    pub bottom_right: f32,
}

impl BorderRadius {
    pub fn circle() -> Self {
        Self {
            top_right: 50.,
            top_left: 50.,
            bottom_left: 50.,
            bottom_right: 50.,
        }
    }
}

impl From<BorderRadius> for [f32; 4] {
    fn from(value: BorderRadius) -> Self {
        [
            value.bottom_right,
            value.top_right,
            value.bottom_left,
            value.top_left,
        ]
    }
}

#[derive(Clone)]
pub struct Border {
    pub size: Insets,
    pub radius: BorderRadius,
    pub color: Color,
}

impl Default for Border {
    fn default() -> Self {
        Self {
            size: Insets::size(1.),
            radius: BorderRadius::default(),
            color: Color {
                urgency_low: [166, 227, 161, 255],
                urgency_normal: [203, 166, 247, 255],
                urgency_critical: [243, 139, 168, 255],
            },
        }
    }
}

#[derive(Clone)]
pub struct Icon {
    pub border: Border,
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
pub struct ButtonState {
    pub background: Color,
    pub border: Border,
    pub font: Font,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            background: Color::rgba([192, 202, 245, 255]),
            border: Border {
                size: Insets::size(0.),
                radius: BorderRadius::circle(),
                ..Default::default()
            },
            font: Font {
                color: Color::rgba([47, 53, 73, 255]),
                ..Default::default()
            },
        }
    }
}

impl ButtonState {
    pub fn default_hover() -> Self {
        Self {
            background: Color::rgba([255, 255, 255, 255]),
            ..Default::default()
        }
    }

    pub fn transparent() -> Self {
        Self {
            background: Color::rgba([0, 0, 0, 0]),
            border: Border {
                size: Insets::size(0.),
                radius: BorderRadius::circle(),
                color: Color::rgba([0, 0, 0, 0]),
            },
            font: Font {
                color: Color::rgba([0, 0, 0, 0]),
                ..Default::default()
            },
        }
    }
}

#[derive(Clone)]
pub struct Button {
    pub default: ButtonState,
    pub hover: ButtonState,
}

impl Default for Button {
    fn default() -> Self {
        Self {
            default: ButtonState::default(),
            hover: ButtonState::default_hover(),
        }
    }
}

impl Button {
    pub fn default_action() -> Self {
        let default = ButtonState {
            font: Font::default(),
            background: Color::rgba([22, 22, 30, 0]),
            border: Border::default(),
        };

        Self {
            default: default.clone(),
            hover: ButtonState {
                background: Color::rgba([247, 118, 142, 255]),
                ..default
            },
        }
    }

    pub fn default_dismiss_unfocused() -> Self {
        Self {
            default: ButtonState::transparent(),
            hover: ButtonState::default_hover(),
        }
    }
}

#[derive(Clone)]
pub struct Buttons {
    pub dismiss: Button,
    pub action: Button,
}

impl Default for Buttons {
    fn default() -> Self {
        Self {
            dismiss: Button::default(),
            action: Button::default_action(),
        }
    }
}

impl Buttons {
    pub fn unfocused() -> Self {
        Self {
            dismiss: Button::default_dismiss_unfocused(),
            action: Button::default_action(),
        }
    }
}

#[derive(Clone)]
pub struct TextStyle {
    pub size: u32,
    pub family: Arc<str>,
    pub color: Color,
    pub border: Border,
    pub background: Color,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            size: 10,
            family: "DejaVu Sans".into(),
            color: Color::rgba([255, 255, 255, 255]),
            border: Border {
                size: Insets::default(),
                ..Default::default()
            },
            background: Color::rgba([0, 0, 0, 0]),
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
    pub summary: TextStyle,
    pub body: TextStyle,
}

impl Default for StyleState {
    fn default() -> Self {
        Self {
            body: TextStyle::default(),
            summary: TextStyle::default(),
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

impl StyleState {
    pub fn default_hover() -> Self {
        Self {
            background: Color::rgba([47, 53, 73, 255]),
            ..Default::default()
        }
    }

    pub fn unfocused() -> Self {
        Self {
            buttons: Buttons::unfocused(),
            ..Default::default()
        }
    }
}

pub struct UrgencyStyles {
    pub focused: StyleState,
    pub unfocused: StyleState,
}

impl Default for UrgencyStyles {
    fn default() -> Self {
        Self {
            focused: StyleState::default_hover(),
            unfocused: StyleState::unfocused(),
        }
    }
}

#[derive(Clone)]
pub struct NotificationCounter {
    pub format: String,
    pub background: Color,
    pub border: Border,
    pub font: Font,
}

impl Default for NotificationCounter {
    fn default() -> Self {
        Self {
            format: "{} more".to_string(),
            background: Color::rgba([30, 30, 46, 200]),
            border: Border::default(),
            font: Font::default(),
        }
    }
}

pub struct Styles {
    pub urgency_low: UrgencyStyles,
    pub urgency_normal: UrgencyStyles,
    pub urgency_critical: UrgencyStyles,
    pub next: NotificationCounter,
    pub prev: NotificationCounter,
}

impl Default for Styles {
    fn default() -> Self {
        Self {
            urgency_low: UrgencyStyles::default(),
            urgency_normal: UrgencyStyles::default(),
            urgency_critical: UrgencyStyles::default(),
            next: NotificationCounter::default(),
            prev: NotificationCounter::default(),
        }
    }
}

impl Styles {
    pub fn find_style(&self, urgency: Urgency, focused: bool) -> &StyleState {
        let urgency_styles = match urgency {
            Urgency::Low => &self.urgency_low,
            Urgency::Normal => &self.urgency_normal,
            Urgency::Critical => &self.urgency_critical,
        };
        if focused {
            &urgency_styles.focused
        } else {
            &urgency_styles.unfocused
        }
    }
}
