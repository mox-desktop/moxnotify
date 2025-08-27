use crate::{Moxnotify, config::Queue};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        notification: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let Some(idle_notification) = state.idle_notification.as_ref()
            && idle_notification == notification
        {
            match event {
                ext_idle_notification_v1::Event::Idled => state
                    .notifications
                    .notifications()
                    .iter()
                    .for_each(|notification| {
                        notification.stop_timer(&state.loop_handle);
                    }),
                ext_idle_notification_v1::Event::Resumed => state
                    .notifications
                    .notifications_mut()
                    .iter_mut()
                    .enumerate()
                    .for_each(|(i, notification)| match state.config.general.queue {
                        Queue::FIFO if i == 0 => notification.start_timer(&state.loop_handle),
                        Queue::Unordered => notification.start_timer(&state.loop_handle),
                        Queue::FIFO => {}
                    }),
                _ => (),
            }
        };
    }
}

delegate_noop!(Moxnotify: ext_idle_notifier_v1::ExtIdleNotifierV1);
