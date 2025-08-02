use super::partial::PartialColor;
use crate::Urgency;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub urgency_low: [u8; 4],
    pub urgency_normal: [u8; 4],
    pub urgency_critical: [u8; 4],
}

impl Color {
    pub fn apply(&mut self, partial: &PartialColor) {
        if let Some(urgency_low) = partial.urgency_low {
            self.urgency_low = urgency_low;
        }
        if let Some(urgency_normal) = partial.urgency_normal {
            self.urgency_normal = urgency_normal;
        }
        if let Some(urgency_critical) = partial.urgency_critical {
            self.urgency_critical = urgency_critical;
        }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ColorVisitor;

        impl<'de> Visitor<'de> for ColorVisitor {
            type Value = Color;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "a hex color string, comma-separated hex colors, or a map with urgency keys",
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Color::from_str(v).map_err(de::Error::custom)
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut color = Color::default();
                let parse_hex = |s: String| -> Result<[u8; 4], M::Error> {
                    parse_hex(&s).map_err(de::Error::custom)
                };

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "urgency_low" => color.urgency_low = parse_hex(map.next_value()?)?,
                        "urgency_normal" => color.urgency_normal = parse_hex(map.next_value()?)?,
                        "urgency_critical" => {
                            color.urgency_critical = parse_hex(map.next_value()?)?
                        }
                        _ => {
                            return Err(de::Error::unknown_field(
                                &key,
                                &["urgency_low", "urgency_normal", "urgency_critical"],
                            ));
                        }
                    }
                }

                Ok(color)
            }
        }

        deserializer.deserialize_any(ColorVisitor)
    }
}

impl Color {
    pub fn rgba(rgba: [u8; 4]) -> Self {
        Color {
            urgency_low: rgba,
            urgency_normal: rgba,
            urgency_critical: rgba,
        }
    }

    pub fn get(&self, urgency: &Urgency) -> [u8; 4] {
        match urgency {
            Urgency::Low => self.urgency_low,
            Urgency::Normal => self.urgency_normal,
            Urgency::Critical => self.urgency_critical,
        }
    }

    pub fn to_linear(self, urgency: &Urgency) -> [f32; 4] {
        let srgb_to_linear = |c: u8| {
            let normalized = c as f32 / 255.0;
            if normalized > 0.04045 {
                ((normalized + 0.055) / 1.055).powf(2.4)
            } else {
                normalized / 12.92
            }
        };

        let rgba = self.get(urgency);

        let r = srgb_to_linear(rgba[0]);
        let g = srgb_to_linear(rgba[1]);
        let b = srgb_to_linear(rgba[2]);
        let a = rgba[3] as f32 / 255.0;

        [r * a, g * a, b * a, a]
    }

    pub fn into_glyphon(self, urgency: &Urgency) -> glyphon::Color {
        let value = match urgency {
            Urgency::Low => self.urgency_low,
            Urgency::Normal => self.urgency_normal,
            Urgency::Critical => self.urgency_critical,
        };

        glyphon::Color::rgba(value[0], value[1], value[2], value[3])
    }
}

impl FromStr for Color {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').map(|part| part.trim()).collect();
        match parts.len() {
            1 => {
                let rgba = parse_hex(parts[0])?;
                Ok(Color {
                    urgency_low: rgba,
                    urgency_normal: rgba,
                    urgency_critical: rgba,
                })
            }
            3 => Ok(Color {
                urgency_low: parse_hex(parts[0])?,
                urgency_normal: parse_hex(parts[1])?,
                urgency_critical: parse_hex(parts[2])?,
            }),
            _ => Err(format!(
                "Invalid number of colors: expected 1 or 3, got {}",
                parts.len()
            )),
        }
    }
}

pub fn parse_hex<T>(hex: T) -> Result<[u8; 4], String>
where
    T: AsRef<str>,
{
    let hex = hex.as_ref();
    if !hex.starts_with('#') {
        return Err("Hex string must start with '#'".to_string());
    }

    let hex_part = &hex[1..];
    let len = hex_part.len();

    if ![3, 4, 6, 8].contains(&len) {
        return Err(format!(
            "Invalid hex length: {len} (expected 3, 4, 6, or 8 characters)"
        ));
    }

    let expanded = match len {
        3 | 4 => {
            let mut expanded = String::with_capacity(8);
            for c in hex_part.chars() {
                expanded.push(c);
                expanded.push(c);
            }
            if len == 3 {
                expanded.push_str("ff");
            }
            expanded
        }
        6 => format!("{hex_part}ff"),
        8 => hex_part.to_string(),
        _ => unreachable!(),
    };

    let parse_component = |start: usize| -> Result<u8, String> {
        u8::from_str_radix(&expanded[start..start + 2], 16)
            .map_err(|e| format!("Invalid hex component: {e}"))
    };

    Ok([
        parse_component(0)?,
        parse_component(2)?,
        parse_component(4)?,
        parse_component(6)?,
    ])
}

impl From<Color> for [f32; 4] {
    fn from(color: Color) -> Self {
        let color = color.urgency_low;
        [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            color[3] as f32 / 255.0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_3_char_hex() {
        let color = Color::from_str("#fff").unwrap();
        assert_eq!(color.urgency_low, [0xff, 0xff, 0xff, 0xff]);
        assert_eq!(color.urgency_normal, [0xff, 0xff, 0xff, 0xff]);
        assert_eq!(color.urgency_critical, [0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn valid_6_char_hex() {
        let color = Color::from_str("#ff0000").unwrap();
        assert_eq!(color.urgency_low, [0xff, 0x00, 0x00, 0xff]);
        assert_eq!(color.urgency_normal, [0xff, 0x00, 0x00, 0xff]);
        assert_eq!(color.urgency_critical, [0xff, 0x00, 0x00, 0xff]);
    }

    #[test]
    fn valid_8_char_hex() {
        let color = Color::from_str("#12345678").unwrap();
        assert_eq!(color.urgency_low, [0x12, 0x34, 0x56, 0x78]);
        assert_eq!(color.urgency_normal, [0x12, 0x34, 0x56, 0x78]);
        assert_eq!(color.urgency_critical, [0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn three_colors() {
        let color = Color::from_str("#fff, #000, #f00").unwrap();
        assert_eq!(color.urgency_low, [0xff, 0xff, 0xff, 0xff]);
        assert_eq!(color.urgency_normal, [0x00, 0x00, 0x00, 0xff]);
        assert_eq!(color.urgency_critical, [0xff, 0x00, 0x00, 0xff]);
    }

    #[test]
    fn invalid_number_of_colors() {
        let result = Color::from_str("#fff,#000");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Invalid number of colors: expected 1 or 3, got 2"
        );
    }

    #[test]
    fn mixed_case_hex() {
        let color = Color::from_str("#FfEeDd").unwrap();
        assert_eq!(color.urgency_low, [0xff, 0xee, 0xdd, 0xff]);
    }

    #[test]
    fn invalid_cases() {
        let test_cases = [
            ("missing hash", "fff", "Hex string must start with '#'"),
            ("invalid_char", "#ggg", "Invalid hex component"),
            ("wrong_length_5", "#12345", "Invalid hex length: 5"),
        ];

        for (name, input, err_msg) in test_cases {
            let result = Color::from_str(input);
            assert!(result.is_err(), "{name} should fail");
            let err = result.unwrap_err();
            assert!(
                err.contains(err_msg),
                "{name}: Expected error containing '{err_msg}', got '{err}'",
            );
        }
    }
}
