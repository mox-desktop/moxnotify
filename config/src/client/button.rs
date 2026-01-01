use super::{Border, BorderRadius, Color, Font, Insets, partial::PartialStyle};

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

#[derive(Clone)]
pub struct Button {
    pub default: ButtonState,
    pub hover: ButtonState,
}

impl Button {
    pub fn apply_hover(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.hover.background.apply(background);
        }

        if let Some(font) = partial.font.as_ref() {
            self.hover.font.apply(font);
        }

        if let Some(border) = partial.border.as_ref() {
            self.hover.border.apply(border);
        }
    }

    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.default.background.apply(background);
            self.hover.background.apply(background);
        }

        if let Some(font) = partial.font.as_ref() {
            self.default.font.apply(font);
            self.hover.font.apply(font);
        }

        if let Some(border) = partial.border.as_ref() {
            self.default.border.apply(border);
            self.hover.border.apply(border);
        }
    }

    fn default_action() -> Self {
        let hover = ButtonState {
            font: Font::default(),
            background: Color::rgba([22, 22, 30, 0]),
            border: Border::default(),
        };

        Self {
            default: hover.clone(),
            hover: ButtonState {
                background: Color::rgba([247, 118, 142, 255]),
                ..hover
            },
        }
    }
}

impl Default for Button {
    fn default() -> Self {
        Self {
            default: ButtonState::default(),
            hover: ButtonState::default_hover(),
        }
    }
}

#[derive(Clone)]
pub struct ButtonState {
    pub background: Color,
    pub border: Border,
    pub font: Font,
}

impl ButtonState {
    fn default_hover() -> Self {
        Self {
            background: Color::rgba([255, 255, 255, 255]),
            ..Default::default()
        }
    }
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            background: Color::rgba([192, 202, 245, 255]),
            border: Border {
                size: Insets {
                    left: 0.,
                    right: 0.,
                    top: 0.,
                    bottom: 0.,
                },
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
