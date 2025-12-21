use super::UiState;
use crate::{
    components::{
        Component,
        notification::Notification,
        text::Text,
    },
    config::Config,
    moxnotify::{common::Urgency, types::{NewNotification, NotificationHints}},
};
use glyphon::{FontSystem, TextArea};
use moxui::shape_renderer;
use std::{
    cell::RefCell,
    ops::Range,
    rc::Rc,
    sync::{Arc, atomic::Ordering},
};

pub struct NotificationView {
    pub visible: Range<usize>,
    pub prev: Option<Notification>,
    pub next: Option<Notification>,
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
        Self {
            visible: 0..config.general.max_visible,
            config,
            font_system,
            prev: None,
            next: None,
            ui_state,
        }
    }

    pub fn update_notification_count(&mut self, notification_count: usize) {
        if self.visible.start > 0 {
            let summary = self
                .config
                .styles
                .next
                .format
                .replace("{}", &self.visible.start.to_string());
            if let Some(notification) = self.prev.as_mut() {
                let mut font_system = self.font_system.borrow_mut();
                notification
                    .summary
                    .as_mut()
                    .expect("Something went horribly wrong")
                    .set_text(&mut font_system, &summary);
            } else {
                self.prev = Some(Notification::counter(
                    Arc::clone(&self.config),
                    &mut self.font_system.borrow_mut(),
                    NewNotification {
                        summary,
                        hints: Some(NotificationHints::default()),
                        ..Default::default()
                    },
                    self.ui_state.clone(),
                ));
            }
        } else {
            self.prev = None;
        }

        if notification_count > self.visible.end {
            let summary = self.config.styles.prev.format.replace(
                "{}",
                &notification_count
                    .saturating_sub(self.visible.end)
                    .to_string(),
            );
            if let Some(notification) = &mut self.next {
                let mut font_system = self.font_system.borrow_mut();
                notification
                    .summary
                    .as_mut()
                    .expect("Something went horribly wrong")
                    .set_text(&mut font_system, &summary);
            } else {
                self.next = Some(Notification::counter(
                    Arc::clone(&self.config),
                    &mut self.font_system.borrow_mut(),
                    NewNotification {
                        summary,
                        hints: Some(NotificationHints::default()),
                        ..Default::default()
                    },
                    self.ui_state.clone(),
                ));
            }
        } else {
            self.next = None;
        }
    }

    pub fn prev_data(
        &self,
        total_width: f32,
    ) -> Option<(shape_renderer::ShapeInstance, TextArea<'_>)> {
        if let Some(prev) = self.prev.as_ref() {
            let extents = prev.get_render_bounds();
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
                prev.summary
                    .as_ref()
                    .expect("Something went horribly wrong")
                    .get_text_areas(Urgency::Low)
                    .swap_remove(0),
            ));
        }

        None
    }

    pub fn next_data(
        &self,
        total_width: f32,
    ) -> Option<(shape_renderer::ShapeInstance, TextArea<'_>)> {
        if let Some(next) = self.next.as_ref() {
            let extents = next.get_render_bounds();
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
                next.summary
                    .as_ref()
                    .expect("Something went horribly wrong")
                    .get_text_areas(Urgency::Low)
                    .swap_remove(0),
            ));
        }

        None
    }
}
