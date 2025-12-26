use crate::{
    Moxnotify,
    config::keymaps::{self, Key, KeyAction, KeyWithModifiers, Keys, Modifiers},
    moxnotify::common::CloseReason,
};
use calloop::{
    RegistrationToken,
    timer::{TimeoutAction, Timer},
};
use std::{sync::atomic::Ordering, time::Duration};
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    protocol::{wl_keyboard, wl_seat},
};
use xkbcommon::xkb::{Context, Keymap, State};

struct Xkb {
    context: Context,
    state: Option<State>,
}

pub struct Keyboard {
    _wl_keyboard: wl_keyboard::WlKeyboard,
    pub repeat: RepeatInfo,
    xkb: Xkb,
    pub key_combination: Keys,
    modifiers: Modifiers,
}

#[derive(Default)]
pub struct RepeatInfo {
    pub key: Option<Key>,
    rate: i32,
    delay: i32,
    registration_token: Option<RegistrationToken>,
}

impl Keyboard {
    pub fn new(qh: &QueueHandle<Moxnotify>, wl_seat: &wl_seat::WlSeat) -> Self {
        let wl_keyboard = wl_seat.get_keyboard(qh, ());

        let xkb_context = Context::new(0);

        Self {
            key_combination: Keys(Vec::new()),
            xkb: Xkb {
                context: xkb_context,
                state: None,
            },
            _wl_keyboard: wl_keyboard,
            repeat: RepeatInfo::default(),
            modifiers: Modifiers::default(),
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: <wl_keyboard::WlKeyboard as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                let keymap_result = unsafe {
                    Keymap::new_from_fd(
                        &state.seat.keyboard.xkb.context,
                        fd,
                        size as usize,
                        format.into(),
                        0,
                    )
                }
                .ok()
                .flatten();

                match keymap_result {
                    Some(keymap) => {
                        let xkb_state = State::new(&keymap);
                        state.seat.keyboard.xkb.state = Some(xkb_state);
                    }
                    None => {
                        log::error!("Keymap data was unexpectedly empty.");
                    }
                }
            }
            wl_keyboard::Event::Modifiers {
                serial: _,
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => {
                if let Some(xkb_state) = state.seat.keyboard.xkb.state.as_mut() {
                    xkb_state.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);

                    let ctrl_active = xkb_state
                        .mod_name_is_active("Control", xkbcommon::xkb::STATE_MODS_EFFECTIVE);
                    let alt_active =
                        xkb_state.mod_name_is_active("Mod1", xkbcommon::xkb::STATE_MODS_EFFECTIVE);
                    let meta_active =
                        xkb_state.mod_name_is_active("Mod4", xkbcommon::xkb::STATE_MODS_EFFECTIVE);

                    state.seat.keyboard.modifiers = Modifiers {
                        control: ctrl_active,
                        alt: alt_active,
                        meta: meta_active,
                    };

                    // Let me handle ctrl, alt and meta my own way while still keeping the shift functionality

                    let shift_active =
                        xkb_state.mod_name_is_active("Shift", xkbcommon::xkb::STATE_MODS_EFFECTIVE);
                    let shift_bit = if shift_active {
                        1 << xkb_state.get_keymap().mod_get_index("Shift")
                    } else {
                        0
                    };

                    xkb_state.update_mask(shift_bit, shift_bit, shift_bit, 0, 0, group);
                }
            }
            wl_keyboard::Event::Key {
                serial: _,
                time: _,
                key,
                state: WEnum::Value(value),
            } => {
                // The wayland protocol gives us an input event code. To convert this to an xkb
                // keycode we must add 8.
                let keycode = key + 8;

                match value {
                    wl_keyboard::KeyState::Released => {
                        state.seat.keyboard.repeat.key = None;
                        if let Some(xkb_state) = state.seat.keyboard.xkb.state.as_ref()
                            && let Some(key) = Key::from_keycode(xkb_state, keycode.into())
                        {
                            let key_with_modifiers = KeyWithModifiers {
                                key,
                                modifiers: state.seat.keyboard.modifiers,
                            };

                            if Keys(vec![key_with_modifiers]) != state.seat.keyboard.key_combination
                            {
                                return;
                            }
                        }

                        if let Some(token) = state.seat.keyboard.repeat.registration_token.take() {
                            state.loop_handle.remove(token);
                        }
                    }
                    wl_keyboard::KeyState::Pressed => {
                        if let Some(xkb_state) = state.seat.keyboard.xkb.state.as_ref() {
                            let key = Key::from_keycode(xkb_state, keycode.into());
                            state.seat.keyboard.repeat.key = key;
                            if let Some(key) = key {
                                let key_with_modifiers = KeyWithModifiers {
                                    key,
                                    modifiers: state.seat.keyboard.modifiers,
                                };
                                state.seat.keyboard.key_combination.push(key_with_modifiers);
                            }

                            if xkb_state.get_keymap().key_repeats(keycode.into()) {
                                if let Some(token) =
                                    state.seat.keyboard.repeat.registration_token.take()
                                {
                                    state.loop_handle.remove(token);
                                }

                                let timer = Timer::from_duration(Duration::from_millis(
                                    state.seat.keyboard.repeat.delay as u64,
                                ));
                                let rate = (1000 / state.seat.keyboard.repeat.rate) as u64;
                                state.seat.keyboard.repeat.registration_token = state
                                    .loop_handle
                                    .insert_source(timer, move |_, (), moxnotify| {
                                        if let Some(key) = moxnotify.seat.keyboard.repeat.key {
                                            let key_with_modifiers = KeyWithModifiers {
                                                key,
                                                modifiers: moxnotify.seat.keyboard.modifiers,
                                            };
                                            moxnotify
                                                .seat
                                                .keyboard
                                                .key_combination
                                                .push(key_with_modifiers);
                                        } else {
                                            return TimeoutAction::Drop;
                                        }

                                        if moxnotify.handle_key().is_err() {
                                            return TimeoutAction::Drop;
                                        }
                                        TimeoutAction::ToDuration(Duration::from_millis(rate))
                                    })
                                    .ok();
                            } else if let Some(token) =
                                state.seat.keyboard.repeat.registration_token
                            {
                                state.loop_handle.remove(token);
                            }
                        }

                        _ = state.handle_key();
                    }
                    _ => unreachable!(),
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                state.seat.keyboard.repeat.delay = delay;
                state.seat.keyboard.repeat.rate = rate;
            }
            _ => {}
        }
    }
}

impl Moxnotify {
    fn handle_key(&mut self) -> anyhow::Result<()> {
        if !self
            .config
            .keymaps
            .matches(&self.seat.keyboard.key_combination)
        {
            let len = self.seat.keyboard.key_combination.len().saturating_sub(1);
            self.seat.keyboard.key_combination.drain(..len);
        }

        log::debug!("key‑combo => {}", self.seat.keyboard.key_combination);

        if let Some(key_combination) = self.config.keymaps.iter().find(|keymap| {
            keymap.keys == self.seat.keyboard.key_combination
                && keymap.mode == self.notifications.ui_state.mode.load(Ordering::Relaxed)
        }) {
            log::debug!("Action executed: {:?}", key_combination.action);
            self.seat.keyboard.key_combination.clear();
            match key_combination.action {
                KeyAction::Noop => {}
                KeyAction::NextNotification => self.notifications.next(),
                KeyAction::PreviousNotification => self.notifications.prev(),
                KeyAction::FirstNotification => self.notifications.first(),
                KeyAction::LastNotification => self.notifications.last(),
                KeyAction::DismissNotification => {
                    if let Some(id) = self.notifications.selected_id() {
                        self.dismiss_with_reason(id, Some(CloseReason::ReasonDismissedByUser));
                        return Ok(());
                    }
                }
                KeyAction::Unfocus => {
                    if let Some(surface) = self.surface.as_mut() {
                        surface.unfocus();
                        self.seat.keyboard.key_combination.clear();
                        self.notifications.deselect();
                        self.seat.keyboard.repeat.key = None;
                    }
                }
                KeyAction::HintMode => self
                    .notifications
                    .ui_state
                    .mode
                    .store(keymaps::Mode::Hint, Ordering::Relaxed),
                KeyAction::Uninhibit => self.notifications.uninhibit(),
                KeyAction::Ihibit => self.notifications.inhibit(),
                KeyAction::ToggleInhibit => {
                    if self.notifications.inhibited() {
                        self.notifications.uninhibit();
                    } else {
                        self.notifications.inhibit();
                    }
                }
                KeyAction::Mute => {
                    self.audio.mute();
                }
                KeyAction::Unmute => {
                    self.audio.unmute();
                }
                KeyAction::ToggleMute => {
                    if self.audio.muted() {
                        self.audio.unmute();
                    } else {
                        self.audio.mute();
                    }
                }
                KeyAction::NormalMode => {
                    self.notifications
                        .ui_state
                        .mode
                        .store(keymaps::Mode::Normal, Ordering::Relaxed);
                }
            }
        } else {
            let combination = self.seat.keyboard.key_combination.to_string();
            if let Some(notification) = self.notifications.selected_notification_mut()
                && let Some(buttons) = notification.buttons_mut()
            {
                buttons.hint(&combination);
            }
        }

        self.update_surface_size();
        if let Some(surface) = self.surface.as_mut() {
            _ = surface.render(
                &self.wgpu_state.device,
                &self.wgpu_state.queue,
                &self.notifications,
            );
        }

        Ok(())
    }
}
