use std::fmt;

#[derive(Default)]
pub struct ViewRange {
    max_visible: usize,
    start: usize,
    end: usize,
}

impl ViewRange {
    pub fn new(max_visible: usize) -> Self {
        Self {
            max_visible,
            ..Default::default()
        }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn width(&self) -> usize {
        self.end - self.start
    }

    pub fn scroll_down_clamped(&mut self, len: usize) {
        if len == 0 {
            self.start = 0;
            self.end = 0;
            return;
        }

        if self.end > len {
            self.start = self.start.saturating_sub(1);
            self.end = len;
        } else if self.start == 0 && self.end - self.start > 1 {
            self.end -= 1;
        } else if self.start > 0 {
            self.start -= 1;
            self.end -= 1;
        }
    }

    pub fn show_tail(&mut self, len: usize) {
        self.start = len.saturating_sub(self.max_visible);
        self.end = len;
    }

    pub fn show_head(&mut self) {
        self.start = 0;
        self.end = self.max_visible;
    }

    pub fn ensure_visible_down(&mut self, index: usize) {
        if index == 0 {
            self.start = 0;
            self.end = self.max_visible;
        } else if index >= self.end {
            self.end = index + 1;
            self.start = self.end.saturating_sub(self.max_visible);
        }
    }

    pub fn ensure_visible_up(&mut self, index: usize, len: usize) {
        if index + 1 == len {
            self.end = len;
            self.start = self.end.saturating_sub(self.max_visible);
        } else if index < self.start {
            self.start = index;
            self.end = self.start + self.max_visible;
        }
    }
}

impl fmt::Display for ViewRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ViewRange {{ {}..{} }}", self.start, self.end)
    }
}
