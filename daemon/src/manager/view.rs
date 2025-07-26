use super::UiState;
use crate::{
    NotificationData,
    components::{Component, notification::Notification, text::Text},
    config::Config,
    utils::buffers,
};
use glyphon::{FontSystem, TextArea};
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

    pub fn prev(&mut self, total_height: f32, index: usize, notification_count: usize) {
        if index + 1 == notification_count {
            self.visible = (notification_count
                .max(self.config.general.max_visible)
                .saturating_sub(self.config.general.max_visible))
                ..notification_count.max(self.config.general.max_visible);
        } else {
            let first_visible = self.visible.start;
            if index < first_visible {
                let start = index;
                let end = index + self.config.general.max_visible;
                self.visible = start..end;
            }
        }
        self.update_notification_count(total_height, notification_count);
    }

    pub fn next(&mut self, total_height: f32, index: usize, notification_count: usize) {
        if index == 0 {
            self.visible = 0..self.config.general.max_visible;
        } else {
            let last_visible = self.visible.end.saturating_sub(1);
            if index > last_visible {
                let start = index + 1 - self.config.general.max_visible;
                let end = index + 1;
                self.visible = start..end;
            }
        }
        self.update_notification_count(total_height, notification_count);
    }

    pub fn update_notification_count(&mut self, mut total_height: f32, notification_count: usize) {
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
                notification.set_position(0., 0.);
            } else {
                self.prev = Some(Notification::new(
                    Arc::clone(&self.config),
                    &mut self.font_system.borrow_mut(),
                    NotificationData {
                        summary: summary.into(),
                        ..Default::default()
                    },
                    self.ui_state.clone(),
                    None,
                ));

                total_height += self
                    .prev
                    .as_ref()
                    .map(|p| p.get_bounds().height)
                    .unwrap_or_default();
            }
        } else {
            total_height -= self
                .prev
                .as_ref()
                .map(|p| p.get_bounds().height)
                .unwrap_or_default();
            self.prev = None;
        };

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
                notification.set_position(
                    notification.x,
                    total_height - notification.get_bounds().height,
                );
            } else {
                let mut next = Notification::new(
                    Arc::clone(&self.config),
                    &mut self.font_system.borrow_mut(),
                    NotificationData {
                        summary: summary.into(),
                        ..Default::default()
                    },
                    self.ui_state.clone(),
                    None,
                );
                next.set_position(next.x, total_height);
                self.next = Some(next);
            }
        } else {
            self.next = None;
        }
    }

    pub fn prev_data(&self, total_width: f32) -> Option<(buffers::Instance, TextArea<'_>)> {
        if let Some(prev) = self.prev.as_ref() {
            let extents = prev.get_render_bounds();
            let style = &self.config.styles.prev;
            let instance = buffers::Instance {
                rect_pos: [extents.x, extents.y],
                rect_size: [
                    total_width - style.border.size.left - style.border.size.right,
                    extents.height - style.border.size.top - style.border.size.bottom,
                ],
                rect_color: style.background.to_linear(&crate::Urgency::Low),
                border_radius: style.border.radius.into(),
                border_size: style.border.size.into(),
                border_color: style.border.color.to_linear(&crate::Urgency::Low),
                scale: self.ui_state.scale.load(Ordering::Relaxed),
                depth: 0.9,
            };

            return Some((
                instance,
                prev.summary
                    .as_ref()
                    .expect("Something went horribly wrong")
                    .get_text_areas(&crate::Urgency::Low)
                    .swap_remove(0),
            ));
        }

        None
    }

    pub fn next_data(&self, total_width: f32) -> Option<(buffers::Instance, TextArea<'_>)> {
        if let Some(next) = self.next.as_ref() {
            let extents = next.get_render_bounds();
            let style = &self.config.styles.prev;
            let instance = buffers::Instance {
                rect_pos: [extents.x, extents.y],
                rect_size: [
                    total_width - style.border.size.left - style.border.size.right,
                    extents.height - style.border.size.top - style.border.size.bottom,
                ],
                rect_color: style.background.to_linear(&crate::Urgency::Low),
                border_radius: style.border.radius.into(),
                border_size: style.border.size.into(),
                border_color: style.border.color.to_linear(&crate::Urgency::Low),
                scale: self.ui_state.scale.load(Ordering::Relaxed),
                depth: 0.9,
            };

            return Some((
                instance,
                next.summary
                    .as_ref()
                    .expect("Something went horribly wrong")
                    .get_text_areas(&crate::Urgency::Low)
                    .swap_remove(0),
            ));
        }

        None
    }
}
