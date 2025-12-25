use super::UiState;
use crate::{
    components::{Component, notification::Notification, text::Text},
    config::Config,
    moxnotify::{
        common::Urgency,
        types::{NewNotification, NotificationHints},
    },
};
use glyphon::{FontSystem, TextArea};
use moxui::shape_renderer;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, atomic::Ordering},
};

pub struct NotificationView {
    pub visible: Vec<u32>,
    pub prev: Notification,
    pub next: Notification,
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
            next,
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
    }

    pub fn prev_data(
        &self,
        total_width: f32,
    ) -> Option<(shape_renderer::ShapeInstance, TextArea<'_>)> {
        let extents = self.prev.get_render_bounds();
        let style = &self.config.styles.prev;
        let instance = shape_renderer::ShapeInstance {
            rect_pos: [extents.x, extents.y],
            rect_size: [
                total_width - style.border.size.left - style.border.size.right,
                extents.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.color(Urgency::Low),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.color(Urgency::Low),
            scale: self.ui_state.scale.load(Ordering::Relaxed),
            depth: 0.9,
        };

        return Some((
            instance,
            self.prev
                .summary
                .as_ref()
                .expect("Something went horribly wrong")
                .get_text_areas(Urgency::Low)
                .swap_remove(0),
        ));
    }

    pub fn next_data(
        &self,
        total_width: f32,
    ) -> Option<(shape_renderer::ShapeInstance, TextArea<'_>)> {
        let extents = self.next.get_render_bounds();
        let style = &self.config.styles.prev;
        let instance = shape_renderer::ShapeInstance {
            rect_pos: [extents.x, extents.y],
            rect_size: [
                total_width - style.border.size.left - style.border.size.right,
                extents.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.color(Urgency::Low),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.color(Urgency::Low),
            scale: self.ui_state.scale.load(Ordering::Relaxed),
            depth: 0.9,
        };

        return Some((
            instance,
            self.next
                .summary
                .as_ref()
                .expect("Something went horribly wrong")
                .get_text_areas(Urgency::Low)
                .swap_remove(0),
        ));
    }
}
