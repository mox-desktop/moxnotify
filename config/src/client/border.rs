use super::{
    Insets,
    color::Color,
    partial::{PartialBorder, PartialBorderRadius},
};

#[derive(Clone)]
pub struct Border {
    pub size: Insets,
    pub radius: BorderRadius,
    pub color: Color,
}

impl Border {
    pub fn apply(&mut self, partial: &PartialBorder) {
        if let Some(color) = partial.color.as_ref() {
            self.color.apply(color);
        }
        if let Some(radius) = partial.radius.as_ref() {
            self.radius.apply(radius);
        }
        if let Some(size) = partial.size.as_ref() {
            self.size.apply(size);
        }
    }
}

impl Default for Border {
    fn default() -> Self {
        Self {
            size: Insets {
                left: 1.,
                right: 1.,
                top: 1.,
                bottom: 1.,
            },
            radius: BorderRadius::default(),
            color: Color {
                urgency_low: [166, 227, 161, 255],
                urgency_normal: [203, 166, 247, 255],
                urgency_critical: [243, 139, 168, 255],
            },
        }
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
    pub fn apply(&mut self, partial: &PartialBorderRadius) {
        if let Some(top_left) = partial.top_left {
            self.top_left = top_left;
        }
        if let Some(top_right) = partial.top_right {
            self.top_right = top_right;
        }
        if let Some(bottom_left) = partial.bottom_left {
            self.bottom_left = bottom_left;
        }
        if let Some(bottom_right) = partial.bottom_right {
            self.bottom_right = bottom_right;
        }
    }
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
