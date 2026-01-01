use crate::client::color::{Color, parse_hex};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use std::{fmt, str::FromStr, sync::Arc};

#[derive(Deserialize, Default, Clone)]
pub struct PartialStyle {
    pub background: Option<PartialColor>,
    pub font: Option<PartialFont>,
    pub border: Option<PartialBorder>,
}

#[derive(Debug, Clone, Default)]
pub struct PartialColor {
    pub urgency_low: Option<[u8; 4]>,
    pub urgency_normal: Option<[u8; 4]>,
    pub urgency_critical: Option<[u8; 4]>,
}

impl<'de> Deserialize<'de> for PartialColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PartialColorVisitor;

        impl<'de> Visitor<'de> for PartialColorVisitor {
            type Value = PartialColor;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "a hex color string, comma-separated hex colors, or a map with urgency keys",
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let color = Color::from_str(v).map_err(de::Error::custom)?;
                Ok(PartialColor {
                    urgency_low: Some(color.urgency_low),
                    urgency_normal: Some(color.urgency_normal),
                    urgency_critical: Some(color.urgency_critical),
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut partial_color = PartialColor::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "urgency_low" => {
                            let hex = map.next_value::<String>()?;
                            let color = parse_hex(&hex).map_err(de::Error::custom)?;
                            partial_color.urgency_low = Some(color);
                        }
                        "urgency_normal" => {
                            let hex = map.next_value::<String>()?;
                            let color = parse_hex(&hex).map_err(de::Error::custom)?;
                            partial_color.urgency_normal = Some(color);
                        }
                        "urgency_critical" => {
                            let hex = map.next_value::<String>()?;
                            let color = parse_hex(&hex).map_err(de::Error::custom)?;
                            partial_color.urgency_critical = Some(color);
                        }
                        _ => {
                            return Err(de::Error::unknown_field(
                                &key,
                                &["urgency_low", "urgency_normal", "urgency_critical"],
                            ));
                        }
                    }
                }
                Ok(partial_color)
            }
        }

        deserializer.deserialize_any(PartialColorVisitor)
    }
}

#[derive(Deserialize, Clone)]
pub struct PartialFont {
    pub size: Option<f32>,
    pub family: Option<Arc<str>>,
    pub color: Option<PartialColor>,
}

#[derive(Default, Clone, Copy)]
pub struct PartialInsets {
    pub left: Option<f32>,
    pub right: Option<f32>,
    pub top: Option<f32>,
    pub bottom: Option<f32>,
}

impl PartialInsets {
    pub fn size(value: f32) -> Self {
        Self {
            left: Some(value),
            right: Some(value),
            top: Some(value),
            bottom: Some(value),
        }
    }
}

impl<'de> Deserialize<'de> for PartialInsets {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PartialInsetsVisitor;

        impl<'de> serde::de::Visitor<'de> for PartialInsetsVisitor {
            type Value = PartialInsets;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number, 'auto', or a map with inset values")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v == "auto" {
                    Ok(PartialInsets::size(0.))
                } else {
                    Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(v),
                        &"auto or number",
                    ))
                }
            }

            fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E> {
                Ok(PartialInsets::size(v))
            }

            fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E> {
                Ok(PartialInsets::size(v as f32))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                Ok(PartialInsets::size(v as f32))
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E> {
                Ok(PartialInsets::size(v as f32))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                Ok(PartialInsets::size(v as f32))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut left = None;
                let mut right = None;
                let mut top = None;
                let mut bottom = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "left" => left = Some(map.next_value()?),
                        "right" => right = Some(map.next_value()?),
                        "top" => top = Some(map.next_value()?),
                        "bottom" => bottom = Some(map.next_value()?),
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &["left", "right", "top", "bottom"],
                            ));
                        }
                    }
                }

                Ok(PartialInsets {
                    left,
                    right,
                    top,
                    bottom,
                })
            }
        }

        deserializer.deserialize_any(PartialInsetsVisitor)
    }
}

#[derive(Deserialize, Clone)]
pub struct PartialBorder {
    pub size: Option<PartialInsets>,
    pub radius: Option<PartialBorderRadius>,
    pub color: Option<PartialColor>,
}

#[derive(Default, Clone, Copy)]
pub struct PartialBorderRadius {
    pub top_left: Option<f32>,
    pub top_right: Option<f32>,
    pub bottom_left: Option<f32>,
    pub bottom_right: Option<f32>,
}

impl<'de> Deserialize<'de> for PartialBorderRadius {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PartialBorderRadiusVisitor;

        impl<'de> serde::de::Visitor<'de> for PartialBorderRadiusVisitor {
            type Value = PartialBorderRadius;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or a map with optional corner values")
            }

            fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E> {
                Ok(PartialBorderRadius {
                    top_left: Some(value),
                    top_right: Some(value),
                    bottom_left: Some(value),
                    bottom_right: Some(value),
                })
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(PartialBorderRadius {
                    top_left: Some(value),
                    top_right: Some(value),
                    bottom_left: Some(value),
                    bottom_right: Some(value),
                })
            }

            fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(PartialBorderRadius {
                    top_left: Some(value),
                    top_right: Some(value),
                    bottom_left: Some(value),
                    bottom_right: Some(value),
                })
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(PartialBorderRadius {
                    top_left: Some(value),
                    top_right: Some(value),
                    bottom_left: Some(value),
                    bottom_right: Some(value),
                })
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(PartialBorderRadius {
                    top_left: Some(value),
                    top_right: Some(value),
                    bottom_left: Some(value),
                    bottom_right: Some(value),
                })
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(PartialBorderRadius {
                    top_left: Some(value),
                    top_right: Some(value),
                    bottom_left: Some(value),
                    bottom_right: Some(value),
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut top_left = None;
                let mut top_right = None;
                let mut bottom_left = None;
                let mut bottom_right = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "top_left" => top_left = Some(map.next_value()?),
                        "top_right" => top_right = Some(map.next_value()?),
                        "bottom_left" => bottom_left = Some(map.next_value()?),
                        "bottom_right" => bottom_right = Some(map.next_value()?),
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &["top_left", "top_right", "bottom_left", "bottom_right"],
                            ));
                        }
                    }
                }

                Ok(PartialBorderRadius {
                    top_left,
                    top_right,
                    bottom_left,
                    bottom_right,
                })
            }
        }

        deserializer.deserialize_any(PartialBorderRadiusVisitor)
    }
}
