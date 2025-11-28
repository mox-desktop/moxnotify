use super::UiState;
use crate::{
    NotificationData,
    components::{
        Component,
        notification::{Notification, Ready},
        text::Text,
    },
    config::Config,
    utils::taffy::NodeContext,
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
    pub prev: Option<Notification<Ready>>,
    pub next: Option<Notification<Ready>>,
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

    pub fn update_notification_count(
        &mut self,
        tree: &mut taffy::TaffyTree<NodeContext>,
        notification_count: usize,
    ) {
        if self.visible.start > 0 {
            let summary = self
                .config
                .styles
                .next
                .format
                .replace("{}", &self.visible.start.to_string());
            if let Some(notification) = self.prev.as_mut() {
                let style = notification.get_style();
                let notification_width = style.width.resolve(0.);

                let mut font_system = self.font_system.borrow_mut();
                let summary_mut = notification
                    .summary
                    .as_mut()
                    .expect("Something went horribly wrong");

                let summary_style = summary_mut.get_style();
                let padding_x = summary_style.padding.left.resolve(0.)
                    + summary_style.padding.right.resolve(0.);
                let border_x = summary_style.border.size.left.resolve(0.)
                    + summary_style.border.size.right.resolve(0.);
                let content_width = notification_width - padding_x - border_x;

                summary_mut.set_text(&mut font_system, &summary);
                summary_mut.set_size(&mut font_system, Some(content_width), None);
                summary_mut
                    .buffer
                    .shape_until_scroll(&mut font_system, false);

                notification.update_layout(tree);
            } else {
                self.prev = Some(Notification::<Ready>::counter(
                    tree,
                    Arc::clone(&self.config),
                    &mut self.font_system.borrow_mut(),
                    NotificationData {
                        summary: summary.into(),
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
                let style = notification.get_style();
                let notification_width = style.width.resolve(0.);

                let mut font_system = self.font_system.borrow_mut();
                let summary_mut = notification
                    .summary
                    .as_mut()
                    .expect("Something went horribly wrong");

                let summary_style = summary_mut.get_style();
                let padding_x = summary_style.padding.left.resolve(0.)
                    + summary_style.padding.right.resolve(0.);
                let border_x = summary_style.border.size.left.resolve(0.)
                    + summary_style.border.size.right.resolve(0.);
                let content_width = notification_width - padding_x - border_x;

                summary_mut.set_text(&mut font_system, &summary);
                summary_mut.set_size(&mut font_system, Some(content_width), None);
                summary_mut
                    .buffer
                    .shape_until_scroll(&mut font_system, false);

                notification.update_layout(tree);
            } else {
                self.next = Some(Notification::<Ready>::counter(
                    tree,
                    Arc::clone(&self.config),
                    &mut self.font_system.borrow_mut(),
                    NotificationData {
                        summary: summary.into(),
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
        tree: &taffy::TaffyTree<NodeContext>,
    ) -> Option<(shape_renderer::ShapeInstance, TextArea<'_>)> {
        if let Some(prev) = self.prev.as_ref() {
            let render_bounds = prev.get_render_bounds(tree);
            let counter_style = &self.config.styles.prev;
            let notification_style = prev.get_style();
            let instance = shape_renderer::ShapeInstance {
                rect_pos: [render_bounds.x, render_bounds.y],
                rect_size: [
                    render_bounds.width
                        - notification_style.border.size.left.resolve(0.)
                        - notification_style.border.size.right.resolve(0.),
                    render_bounds.height
                        - notification_style.border.size.top.resolve(0.)
                        - notification_style.border.size.bottom.resolve(0.),
                ],
                rect_color: counter_style.background.color(crate::Urgency::Low),
                border_radius: counter_style.border.radius.into(),
                border_size: counter_style.border.size.into(),
                border_color: counter_style.border.color.color(crate::Urgency::Low),
                scale: self.ui_state.scale.load(Ordering::Relaxed),
                depth: 0.9,
            };

            return Some((
                instance,
                prev.summary
                    .as_ref()
                    .expect("Something went horribly wrong")
                    .get_text_areas(tree, crate::Urgency::Low)
                    .swap_remove(0),
            ));
        }

        None
    }

    pub fn next_data(
        &self,
        tree: &taffy::TaffyTree<NodeContext>,
    ) -> Option<(shape_renderer::ShapeInstance, TextArea<'_>)> {
        if let Some(next) = self.next.as_ref() {
            let render_bounds = next.get_render_bounds(tree);
            let counter_style = &self.config.styles.next;
            let notification_style = next.get_style();
            let instance = shape_renderer::ShapeInstance {
                rect_pos: [render_bounds.x, render_bounds.y],
                rect_size: [
                    render_bounds.width
                        - notification_style.border.size.left.resolve(0.)
                        - notification_style.border.size.right.resolve(0.),
                    render_bounds.height
                        - notification_style.border.size.top.resolve(0.)
                        - notification_style.border.size.bottom.resolve(0.),
                ],
                rect_color: counter_style.background.color(crate::Urgency::Low),
                border_radius: counter_style.border.radius.into(),
                border_size: counter_style.border.size.into(),
                border_color: counter_style.border.color.color(crate::Urgency::Low),
                scale: self.ui_state.scale.load(Ordering::Relaxed),
                depth: 0.9,
            };

            return Some((
                instance,
                next.summary
                    .as_ref()
                    .expect("Something went horribly wrong")
                    .get_text_areas(tree, crate::Urgency::Low)
                    .swap_remove(0),
            ));
        }

        None
    }
}
