use serde::{Deserialize, Deserializer, de};
use std::marker::PhantomData;
use std::ops::DerefMut;
use std::sync::atomic::{AtomicU8, Ordering};
use std::{fmt, ops::Deref, str::FromStr};
use xkbcommon::xkb::Keysym;

#[derive(Debug)]
pub struct Keymaps(Vec<KeyCombination>);

impl Keymaps {
    pub fn matches(&self, sequence: &[KeyWithModifiers]) -> bool {
        self.iter().any(|kc| kc.keys.starts_with(sequence))
    }
}

impl<'de> Deserialize<'de> for Keymaps {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let user_keycombs: Vec<KeyCombination> = Vec::deserialize(deserializer)?;

        let mut merged = Self::default().0;

        for kc in user_keycombs {
            if let Some(pos) = merged
                .iter()
                .position(|default_kc| default_kc.mode == kc.mode && default_kc.keys == kc.keys)
            {
                merged[pos] = kc;
            } else {
                merged.push(kc);
            }
        }

        Ok(Keymaps(merged))
    }
}

impl Default for Keymaps {
    fn default() -> Self {
        Self(vec![
            KeyCombination {
                keys: Keys(vec![KeyWithModifiers {
                    key: Key::Character('j'),
                    modifiers: Modifiers::default(),
                }]),
                action: KeyAction::NextNotification,
                mode: Mode::Normal,
            },
            KeyCombination {
                keys: Keys(vec![KeyWithModifiers {
                    key: Key::Character('k'),
                    modifiers: Modifiers::default(),
                }]),
                mode: Mode::Normal,
                action: KeyAction::PreviousNotification,
            },
            KeyCombination {
                keys: Keys(vec![KeyWithModifiers {
                    key: Key::Character('x'),
                    modifiers: Modifiers::default(),
                }]),
                action: KeyAction::DismissNotification,
                mode: Mode::Normal,
            },
            KeyCombination {
                keys: Keys(vec![
                    KeyWithModifiers {
                        key: Key::Character('d'),
                        modifiers: Modifiers::default(),
                    },
                    KeyWithModifiers {
                        key: Key::Character('d'),
                        modifiers: Modifiers::default(),
                    },
                ]),
                action: KeyAction::DismissNotification,
                mode: Mode::Normal,
            },
            KeyCombination {
                keys: Keys(vec![KeyWithModifiers {
                    key: Key::Character('G'),
                    modifiers: Modifiers::default(),
                }]),
                action: KeyAction::LastNotification,
                mode: Mode::Normal,
            },
            KeyCombination {
                keys: Keys(vec![
                    KeyWithModifiers {
                        key: Key::Character('g'),
                        modifiers: Modifiers::default(),
                    },
                    KeyWithModifiers {
                        key: Key::Character('g'),
                        modifiers: Modifiers::default(),
                    },
                ]),
                mode: Mode::Normal,
                action: KeyAction::FirstNotification,
            },
            KeyCombination {
                keys: Keys(vec![KeyWithModifiers {
                    key: Key::SpecialKey(SpecialKeyCode::Escape),
                    modifiers: Modifiers::default(),
                }]),
                mode: Mode::Hint,
                action: KeyAction::NormalMode,
            },
            KeyCombination {
                keys: Keys(vec![KeyWithModifiers {
                    key: Key::SpecialKey(SpecialKeyCode::Escape),
                    modifiers: Modifiers::default(),
                }]),
                action: KeyAction::Unfocus,
                mode: Mode::Normal,
            },
            KeyCombination {
                keys: Keys(vec![KeyWithModifiers {
                    key: Key::Character('f'),
                    modifiers: Modifiers::default(),
                }]),
                action: KeyAction::HintMode,
                mode: Mode::Normal,
            },
            KeyCombination {
                keys: Keys(vec![KeyWithModifiers {
                    key: Key::Character('h'),
                    modifiers: Modifiers::default(),
                }]),
                action: KeyAction::ToggleHistory,
                mode: Mode::Normal,
            },
            KeyCombination {
                keys: Keys(vec![KeyWithModifiers {
                    key: Key::Character('m'),
                    modifiers: Modifiers::default(),
                }]),
                action: KeyAction::ToggleMute,
                mode: Mode::Normal,
            },
            KeyCombination {
                keys: Keys(vec![KeyWithModifiers {
                    key: Key::Character('i'),
                    modifiers: Modifiers::default(),
                }]),
                action: KeyAction::ToggleInhibit,
                mode: Mode::Normal,
            },
        ])
    }
}

impl Deref for Keymaps {
    type Target = Vec<KeyCombination>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Default, Clone, Copy)]
#[repr(u8)]
pub enum Mode {
    #[default]
    #[serde(rename = "n")]
    Normal = 0,
    #[serde(rename = "h")]
    Hint = 1,
}

pub struct AtomicMode {
    inner: AtomicU8,
}

impl AtomicMode {
    pub fn new(mode: Mode) -> Self {
        Self {
            inner: AtomicU8::new(mode as u8),
        }
    }

    pub fn load(&self, ordering: Ordering) -> Mode {
        match self.inner.load(ordering) {
            0 => Mode::Normal,
            1 => Mode::Hint,
            _ => unreachable!("Invalid Mode value"),
        }
    }

    pub fn store(&self, mode: Mode, ordering: Ordering) {
        self.inner.store(mode as u8, ordering);
    }

    pub fn swap(&self, mode: Mode, ordering: Ordering) -> Mode {
        let old = self.inner.swap(mode as u8, ordering);
        match old {
            0 => Mode::Normal,
            1 => Mode::Hint,
            _ => unreachable!("Invalid Mode value"),
        }
    }

    pub fn compare_exchange(
        &self,
        current: Mode,
        new: Mode,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Mode, Mode> {
        match self
            .inner
            .compare_exchange(current as u8, new as u8, success, failure)
        {
            Ok(old) => Ok(match old {
                0 => Mode::Normal,
                1 => Mode::Hint,
                _ => unreachable!(),
            }),
            Err(old) => Err(match old {
                0 => Mode::Normal,
                1 => Mode::Hint,
                _ => unreachable!(),
            }),
        }
    }
}

impl Default for AtomicMode {
    fn default() -> Self {
        Self::new(Mode::default())
    }
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "normal" => Ok(Mode::Normal),
            "hint" => Ok(Mode::Hint),
            _ => Err(format!("Invalid mode: {s}")),
        }
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Default, Clone, Copy)]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(PartialEq, Debug)]
pub struct Keys(pub Vec<KeyWithModifiers>);

impl fmt::Display for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<String>()
        )
    }
}

impl Deref for Keys {
    type Target = Vec<KeyWithModifiers>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Keys {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Deserialize, PartialEq, Debug)]
pub struct KeyCombination {
    pub mode: Mode,
    pub keys: Keys,
    pub action: KeyAction,
}

impl<'de> Deserialize<'de> for Keys {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct KeysVisitor;

        impl<'de> de::Visitor<'de> for KeysVisitor {
            type Value = Keys;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or a sequence of key combinations")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let mut keys = Vec::new();
                let mut remaining = s;

                while !remaining.is_empty() {
                    let mut parsed = false;
                    for len in (1..=remaining.len()).rev() {
                        let candidate = &remaining[..len];
                        if let Ok(key) = KeyWithModifiers::from_str(candidate) {
                            keys.push(key);
                            remaining = &remaining[len..];
                            parsed = true;
                            break;
                        }
                    }
                    if !parsed {
                        return Err(de::Error::custom(format!(
                            "Failed to parse key sequence '{s}' at '{remaining}'"
                        )));
                    }
                }

                Ok(Keys(keys))
            }

            fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
            where
                S: de::SeqAccess<'de>,
            {
                let mut keys = Vec::new();
                while let Some(s) = seq.next_element::<String>()? {
                    let key = KeyWithModifiers::from_str(&s).map_err(de::Error::custom)?;
                    keys.push(key);
                }
                Ok(Keys(keys))
            }
        }

        deserializer.deserialize_any(KeysVisitor)
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Default)]
pub struct KeyWithModifiers {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl fmt::Display for KeyWithModifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut result = String::new();

        if self.modifiers.control {
            result.push_str("C-");
        }
        if self.modifiers.alt {
            result.push_str("M-");
        }
        if self.modifiers.meta {
            result.push_str("S-");
        }

        match self.key {
            Key::Character(c) => {
                if c == ' ' {
                    result.push_str("<Space>");
                } else {
                    result.push(c);
                }
            }
            Key::SpecialKey(special) => {
                result.push_str(&format!(
                    "<{}>",
                    match special {
                        SpecialKeyCode::Enter => "CR",
                        SpecialKeyCode::Backspace => "BS",
                        SpecialKeyCode::Tab => "Tab",
                        SpecialKeyCode::Space => "Space",
                        SpecialKeyCode::Escape => "Esc",
                        SpecialKeyCode::Up => "Up",
                        SpecialKeyCode::Down => "Down",
                        SpecialKeyCode::Left => "Left",
                        SpecialKeyCode::Right => "Right",
                        SpecialKeyCode::Home => "Home",
                        SpecialKeyCode::End => "End",
                        SpecialKeyCode::PageUp => "PageUp",
                        SpecialKeyCode::PageDown => "PageDown",
                        SpecialKeyCode::Insert => "Insert",
                        SpecialKeyCode::Delete => "Delete",
                        SpecialKeyCode::F1 => "F1",
                        SpecialKeyCode::F2 => "F2",
                        SpecialKeyCode::F3 => "F3",
                        SpecialKeyCode::F4 => "F4",
                        SpecialKeyCode::F5 => "F5",
                        SpecialKeyCode::F6 => "F6",
                        SpecialKeyCode::F7 => "F7",
                        SpecialKeyCode::F8 => "F8",
                        SpecialKeyCode::F9 => "F9",
                        SpecialKeyCode::F10 => "F10",
                        SpecialKeyCode::F11 => "F11",
                        SpecialKeyCode::F12 => "F12",
                    }
                ));
            }
        }

        write!(f, "{result}")
    }
}

struct FromStrVisitor<T>(PhantomData<T>);

impl<T> FromStrVisitor<T> {
    fn new() -> Self {
        FromStrVisitor(PhantomData)
    }
}

impl<T> de::Visitor<'_> for FromStrVisitor<T>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string that can be parsed by FromStr")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        T::from_str(value).map_err(|err| de::Error::custom(err))
    }
}

impl<'de> Deserialize<'de> for KeyWithModifiers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(FromStrVisitor::<KeyWithModifiers>::new())
    }
}

impl std::str::FromStr for KeyWithModifiers {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut modifiers = Modifiers {
            control: false,
            alt: false,
            meta: false,
        };

        let mut remaining = s;

        while let Some(rest) = remaining.strip_prefix("<C-") {
            modifiers.control = true;
            remaining = rest.strip_suffix('>').unwrap_or(rest);
        }

        while let Some(rest) = remaining.strip_prefix("<M-") {
            modifiers.alt = true;
            remaining = rest.strip_suffix('>').unwrap_or(rest);
        }

        while let Some(rest) = remaining.strip_prefix("<D-") {
            modifiers.meta = true;
            remaining = rest.strip_suffix('>').unwrap_or(rest);
        }

        let key = if remaining.starts_with('<') && remaining.ends_with('>') {
            let special_key = &remaining[1..remaining.len() - 1];
            match special_key {
                "Space" => Key::SpecialKey(SpecialKeyCode::Space),
                "CR" => Key::SpecialKey(SpecialKeyCode::Enter),
                "BS" => Key::SpecialKey(SpecialKeyCode::Backspace),
                "Tab" => Key::SpecialKey(SpecialKeyCode::Tab),
                "Esc" => Key::SpecialKey(SpecialKeyCode::Escape),
                "Up" => Key::SpecialKey(SpecialKeyCode::Up),
                "Down" => Key::SpecialKey(SpecialKeyCode::Down),
                "Left" => Key::SpecialKey(SpecialKeyCode::Left),
                "Right" => Key::SpecialKey(SpecialKeyCode::Right),
                "Home" => Key::SpecialKey(SpecialKeyCode::Home),
                "End" => Key::SpecialKey(SpecialKeyCode::End),
                "PageUp" => Key::SpecialKey(SpecialKeyCode::PageUp),
                "PageDown" => Key::SpecialKey(SpecialKeyCode::PageDown),
                "Insert" => Key::SpecialKey(SpecialKeyCode::Insert),
                "Delete" => Key::SpecialKey(SpecialKeyCode::Delete),
                "F1" => Key::SpecialKey(SpecialKeyCode::F1),
                "F2" => Key::SpecialKey(SpecialKeyCode::F2),
                "F3" => Key::SpecialKey(SpecialKeyCode::F3),
                "F4" => Key::SpecialKey(SpecialKeyCode::F4),
                "F5" => Key::SpecialKey(SpecialKeyCode::F5),
                "F6" => Key::SpecialKey(SpecialKeyCode::F6),
                "F7" => Key::SpecialKey(SpecialKeyCode::F7),
                "F8" => Key::SpecialKey(SpecialKeyCode::F8),
                "F9" => Key::SpecialKey(SpecialKeyCode::F9),
                "F10" => Key::SpecialKey(SpecialKeyCode::F10),
                "F11" => Key::SpecialKey(SpecialKeyCode::F11),
                "F12" => Key::SpecialKey(SpecialKeyCode::F12),
                _ => return Err(format!("Unknown special key: {special_key}")),
            }
        } else if remaining.len() == 1 {
            Key::Character(remaining.chars().next().unwrap())
        } else {
            return Err(format!("Invalid key format: {remaining}"));
        };

        Ok(KeyWithModifiers { key, modifiers })
    }
}

impl fmt::Display for KeyCombination {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.keys
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<String>()
        )
    }
}

impl KeyCombination {
    pub fn clear(&mut self) {
        self.keys.0.clear();
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum Key {
    Character(char),
    SpecialKey(SpecialKeyCode),
}

impl Key {
    pub fn from_keycode(
        xkb_state: &xkbcommon::xkb::State,
        keycode: xkbcommon::xkb::Keycode,
    ) -> Option<Self> {
        let key_name = xkb_state.key_get_one_sym(keycode);

        if key_name.is_modifier_key() {
            return None;
        }

        match key_name {
            Keysym::Return => Some(Key::SpecialKey(SpecialKeyCode::Enter)),
            Keysym::BackSpace => Some(Key::SpecialKey(SpecialKeyCode::Backspace)),
            Keysym::Tab => Some(Key::SpecialKey(SpecialKeyCode::Tab)),
            Keysym::Escape => Some(Key::SpecialKey(SpecialKeyCode::Escape)),
            Keysym::space => Some(Key::SpecialKey(SpecialKeyCode::Space)),
            Keysym::uparrow => Some(Key::SpecialKey(SpecialKeyCode::Up)),
            Keysym::downarrow => Some(Key::SpecialKey(SpecialKeyCode::Down)),
            Keysym::leftarrow => Some(Key::SpecialKey(SpecialKeyCode::Left)),
            Keysym::rightarrow => Some(Key::SpecialKey(SpecialKeyCode::Right)),
            _ => {
                let key_sym = xkb_state.key_get_one_sym(keycode);
                if u32::from(key_sym) == xkbcommon::xkb::keysyms::KEY_NoSymbol {
                    return None;
                }
                let key_char_code = xkb_state.key_get_utf32(keycode);
                char::from_u32(key_char_code).map(Key::Character)
            }
        }
    }
}

impl Default for Key {
    fn default() -> Self {
        Key::Character('\0')
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum SpecialKeyCode {
    Enter,
    Backspace,
    Tab,
    Space,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    NextNotification,
    PreviousNotification,
    DismissNotification,
    FirstNotification,
    LastNotification,
    Unfocus,
    Noop,
    HintMode,
    NormalMode,
    Mute,
    Unmute,
    ToggleMute,
    Ihibit,
    Uninhibit,
    ToggleInhibit,
    ShowHistory,
    HideHistory,
    ToggleHistory,
}
