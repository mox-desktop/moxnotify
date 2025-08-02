use super::{Border, BorderRadius, Color, Font, Insets, Size, partial::PartialStyle};

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

        if let Some(width) = partial.width.as_ref() {
            self.hover.width = *width;
        }

        if let Some(height) = partial.height.as_ref() {
            self.hover.height = *height;
        }

        if let Some(font) = partial.font.as_ref() {
            self.hover.font.apply(font);
        }

        if let Some(border) = partial.border.as_ref() {
            self.hover.border.apply(border);
        }

        if let Some(margin) = partial.margin.as_ref() {
            self.hover.margin.apply(margin);
        }

        if let Some(padding) = partial.padding.as_ref() {
            self.hover.padding.apply(padding);
        }
    }

    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.default.background.apply(background);
            self.hover.background.apply(background);
        }

        //if let Some(min_width) = partial.min_width.as_ref() {
        //styles.default.buttons.action.default.min_width = *min_width;
        //styles.default.buttons.action.hover.min_width = *min_width;
        //styles.hover.buttons.action.default.min_width = *min_width;
        //styles.hover.buttons.action.hover.min_width = *min_width;
        //}

        if let Some(width) = partial.width.as_ref() {
            self.default.width = *width;
            self.hover.width = *width;
        }

        //if let Some(max_width) = partial.max_width.as_ref() {
        //styles.default.buttons.action.default.max_width = *max_width;
        //styles.default.buttons.action.hover.max_width = *max_width;
        //styles.hover.buttons.action.default.max_width = *max_width;
        //styles.hover.buttons.action.hover.max_width = *max_width;
        //}

        //if let Some(min_height) = partial.min_height.as_ref() {
        //styles.default.buttons.action.default.min_height = *min_height;
        //styles.default.buttons.action.hover.min_height = *min_height;
        //styles.hover.buttons.action.default.min_height = *min_height;
        //styles.hover.buttons.action.hover.min_height = *min_height;
        //}

        if let Some(height) = partial.height.as_ref() {
            self.default.height = *height;
            self.hover.height = *height;
        }

        //if let Some(max_height) = partial.max_height.as_ref() {
        //styles.default.buttons.action.default.max_height = *max_height;
        //styles.default.buttons.action.hover.max_height = *max_height;
        //styles.hover.buttons.action.default.max_height = *max_height;
        //styles.hover.buttons.action.hover.max_height = *max_height;
        //

        if let Some(font) = partial.font.as_ref() {
            self.default.font.apply(font);
            self.hover.font.apply(font);
        }

        if let Some(border) = partial.border.as_ref() {
            self.default.border.apply(border);
            self.hover.border.apply(border);
        }

        if let Some(margin) = partial.margin.as_ref() {
            self.default.margin.apply(margin);
            self.hover.margin.apply(margin);
        }

        if let Some(padding) = partial.padding.as_ref() {
            self.default.padding.apply(padding);
            self.hover.padding.apply(padding);
        }
    }

    fn default_action() -> Self {
        let hover = ButtonState {
            padding: Insets {
                left: Size::Auto,
                right: Size::Auto,
                top: Size::Value(5.),
                bottom: Size::Value(5.),
            },
            margin: Insets {
                left: Size::Value(5.),
                right: Size::Value(5.),
                top: Size::Value(0.),
                bottom: Size::Value(0.),
            },
            width: Size::Auto,
            height: Size::Auto,
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
    pub width: Size,
    pub height: Size,
    pub padding: Insets,
    pub margin: Insets,
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
            padding: Insets {
                left: Size::Auto,
                right: Size::Auto,
                top: Size::Auto,
                bottom: Size::Auto,
            },
            margin: Insets {
                left: Size::Value(0.),
                right: Size::Value(0.),
                top: Size::Value(0.),
                bottom: Size::Value(0.),
            },
            width: Size::Value(20.),
            height: Size::Value(20.),
            background: Color::rgba([192, 202, 245, 255]),
            border: Border {
                size: Insets {
                    left: Size::Value(0.),
                    right: Size::Value(0.),
                    top: Size::Value(0.),
                    bottom: Size::Value(0.),
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
