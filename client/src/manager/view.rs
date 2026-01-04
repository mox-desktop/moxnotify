use super::UiState;
use crate::components::{Component, notification::Notification, text::Text};
use crate::moxnotify::types::{NewNotification, NotificationHints};
use config::client::{ClientConfig as Config, Urgency};
use glyphon::{FontSystem, TextArea};
use moxui::shape_renderer;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, atomic::Ordering},
};

pub struct NotificationView {
    pub visible: Vec<u32>,
    prev: Notification,
    prev_count: u32,
    next: Notification,
    next_count: u32,
    font_system: Rc<RefCell<FontSystem>>,
    config: Arc<Config>,
    ui_state: UiState,
}

impl NotificationView {
    pub fn new(
        config: Arc<Config>,
        ui_state: UiState,
        font_system: Rc<RefCell<FontSystem>>,
    ) -> Self {
        let mut prev = Notification::counter(
            Arc::clone(&config),
            &mut font_system.borrow_mut(),
            NewNotification {
                summary: String::new(),
                hints: Some(NotificationHints::default()),
                ..Default::default()
            },
            ui_state.clone(),
        );
        prev.set_position(0., 0.);

        let next = Notification::counter(
            Arc::clone(&config),
            &mut font_system.borrow_mut(),
            NewNotification {
                summary: String::new(),
                hints: Some(NotificationHints::default()),
                ..Default::default()
            },
            ui_state.clone(),
        );

        Self {
            visible: Vec::new(),
            config: Arc::clone(&config),
            font_system: Rc::clone(&font_system),
            prev,
            prev_count: 0,
            next,
            next_count: 0,
            ui_state,
        }
    }

    pub fn update(&mut self, visible: Vec<u32>, prev: u32, next: u32) {
        self.set_visible(visible);
        self.set_prev(prev);
        self.set_next(next);
    }

    fn set_visible(&mut self, visible: Vec<u32>) {
        self.visible = visible;
    }

    fn set_prev(&mut self, count: u32) {
        let summary = self
            .config
            .styles
            .next
            .format
            .replace("{}", &count.to_string());

        let mut font_system = self.font_system.borrow_mut();
        self.prev
            .summary
            .as_mut()
            .expect("Something went horribly wrong")
            .set_text(&mut font_system, &summary);
        self.prev_count = count;
    }

    fn set_next(&mut self, count: u32) {
        let summary = self
            .config
            .styles
            .prev
            .format
            .replace("{}", &count.to_string());

        let mut font_system = self.font_system.borrow_mut();
        self.next
            .summary
            .as_mut()
            .expect("Something went horribly wrong")
            .set_text(&mut font_system, &summary);
        self.next_count = count;
    }

    pub fn prev_data(
        &self,
        total_width: f32,
    ) -> Option<(shape_renderer::ShapeInstance, TextArea<'_>)> {
        if self.prev_count == 0 {
            return None;
        }

        let extents = self.prev.get_render_bounds();
        let style = &self.config.styles.prev;
        const COUNTER_BORDER_SIZE: f32 = 1.0;
        let instance = shape_renderer::ShapeInstance {
            rect_pos: [extents.x, extents.y],
            rect_size: [
                total_width - COUNTER_BORDER_SIZE * 2.0,
                extents.height - COUNTER_BORDER_SIZE * 2.0,
            ],
            rect_color: style.background.color(Urgency::Low),
            border_radius: style.border.radius.into(),
            border_size: [COUNTER_BORDER_SIZE; 4],
            border_color: style.border.color.color(Urgency::Low),
            scale: self.ui_state.scale.load(Ordering::Relaxed),
            depth: 0.9,
        };

        Some((
            instance,
            self.prev
                .summary
                .as_ref()
                .expect("Something went horribly wrong")
                .get_text_areas(Urgency::Low)
                .swap_remove(0),
        ))
    }

    pub fn next_data(
        &self,
        total_width: f32,
    ) -> Option<(shape_renderer::ShapeInstance, TextArea<'_>)> {
        if self.next_count == 0 {
            return None;
        }

        let extents = self.next.get_render_bounds();
        let style = &self.config.styles.prev;
        const COUNTER_BORDER_SIZE: f32 = 1.0;
        let instance = shape_renderer::ShapeInstance {
            rect_pos: [extents.x, extents.y],
            rect_size: [
                total_width - COUNTER_BORDER_SIZE * 2.0,
                extents.height - COUNTER_BORDER_SIZE * 2.0,
            ],
            rect_color: style.background.color(Urgency::Low),
            border_radius: style.border.radius.into(),
            border_size: [COUNTER_BORDER_SIZE; 4],
            border_color: style.border.color.color(Urgency::Low),
            scale: self.ui_state.scale.load(Ordering::Relaxed),
            depth: 0.9,
        };

        Some((
            instance,
            self.next
                .summary
                .as_ref()
                .expect("Something went horribly wrong")
                .get_text_areas(Urgency::Low)
                .swap_remove(0),
        ))
    }

    /// Get the bounds of the previous notification counter, if notifications exist
    pub fn prev_bounds(&self) -> Option<crate::components::Bounds> {
        if self.prev_count == 0 {
            None
        } else {
            Some(self.prev.get_bounds())
        }
    }

    /// Get the bounds of the next notification counter, if notifications exist
    pub fn next_bounds(&self) -> Option<crate::components::Bounds> {
        if self.next_count == 0 {
            None
        } else {
            Some(self.next.get_bounds())
        }
    }

    /// Set the position of the next notification counter
    pub fn set_next_position(&mut self, x: f32, y: f32) {
        self.next.set_position(x, y);
    }
}
