use super::{Color, Insets, border::Border, partial::PartialStyle};
use std::sync::Arc;

#[derive(Clone)]
pub struct Summary {
    pub size: u32,
    pub family: Arc<str>,
    pub color: Color,
    pub border: Border,
    pub background: Color,
}

impl Summary {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(font) = partial.font.as_ref() {
            if let Some(size) = font.size {
                self.size = size;
            }
            if let Some(family) = font.family.as_ref().map(Arc::clone) {
                self.family = family;
            }
            if let Some(color) = font.color.as_ref() {
                self.color.apply(color);
            }
        }
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
        if let Some(background) = partial.background.as_ref() {
            self.background.apply(background);
        }
    }
}

impl Default for Summary {
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
pub struct Body {
    pub size: u32,
    pub family: Arc<str>,
    pub color: Color,
    pub border: Border,
    pub background: Color,
}

impl Body {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(font) = partial.font.as_ref() {
            if let Some(size) = font.size {
                self.size = size;
            }
            if let Some(family) = font.family.as_ref().map(Arc::clone) {
                self.family = family;
            }
            if let Some(color) = font.color.as_ref() {
                self.color.apply(color);
            }
        }
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
        if let Some(background) = partial.background.as_ref() {
            self.background.apply(background);
        }
    }
}

impl Default for Body {
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
