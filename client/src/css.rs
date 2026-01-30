use simplecss::StyleSheet;

#[derive(Debug, Default, Clone)]
pub struct CssStyles {
    pub notification: NotificationColors,
    pub notification_hover: NotificationColors,
    pub notification_low: NotificationColors,
    pub notification_normal: NotificationColors,
    pub notification_critical: NotificationColors,
    pub summary: TextColors,
    pub body: TextColors,
    pub progress: ProgressColors,
    pub progress_complete: ProgressColors,
    pub button_action: ButtonColors,
    pub button_dismiss: ButtonColors,
    pub hint: HintColors,
}

#[derive(Debug, Default, Clone)]
pub struct NotificationColors {
    pub background: Option<[u8; 4]>,
    pub border_color: Option<[u8; 4]>,
}

#[derive(Debug, Default, Clone)]
pub struct TextColors {
    pub color: Option<[u8; 4]>,
    pub background: Option<[u8; 4]>,
    pub border_color: Option<[u8; 4]>,
}

#[derive(Debug, Default, Clone)]
pub struct ProgressColors {
    pub background: Option<[u8; 4]>,
    pub color: Option<[u8; 4]>,
    pub border_color: Option<[u8; 4]>,
}

#[derive(Debug, Default, Clone)]
pub struct ButtonColors {
    pub background: Option<[u8; 4]>,
    pub color: Option<[u8; 4]>,
    pub border_color: Option<[u8; 4]>,
}

#[derive(Debug, Default, Clone)]
pub struct HintColors {
    pub background: Option<[u8; 4]>,
    pub color: Option<[u8; 4]>,
    pub border_color: Option<[u8; 4]>,
}

pub fn parse_css(css: &str) -> CssStyles {
    let mut styles = CssStyles::default();

    if css.is_empty() {
        return styles;
    }

    let stylesheet = StyleSheet::parse(css);

    for rule in stylesheet.rules {
        let selector = rule.selector.to_string();

        for declaration in rule.declarations {
            let property = declaration.name;
            let value = declaration.value;

            match selector.as_str() {
                ".notification" => {
                    apply_notification_color(&mut styles.notification, &property, value);
                }
                ".notification:hover" => {
                    apply_notification_color(&mut styles.notification_hover, &property, value);
                }
                ".notification.low" | ".notification.low *" => {
                    apply_notification_color(&mut styles.notification_low, &property, value);
                }
                ".notification.normal" | ".notification.normal *" => {
                    apply_notification_color(&mut styles.notification_normal, &property, value);
                }
                ".notification.critical" | ".notification.critical *" => {
                    apply_notification_color(&mut styles.notification_critical, &property, value);
                }
                s if s.starts_with(".notification.low ") => {
                    apply_notification_color(&mut styles.notification_low, &property, value);
                }
                s if s.starts_with(".notification.normal ") => {
                    apply_notification_color(&mut styles.notification_normal, &property, value);
                }
                s if s.starts_with(".notification.critical ") => {
                    apply_notification_color(&mut styles.notification_critical, &property, value);
                }
                ".summary" => {
                    apply_text_color(&mut styles.summary, &property, value);
                }
                ".body" => {
                    apply_text_color(&mut styles.body, &property, value);
                }
                ".progress" => {
                    apply_progress_color(&mut styles.progress, &property, value);
                }
                ".progress-complete" => {
                    apply_progress_color(&mut styles.progress_complete, &property, value);
                }
                ".button-action" => {
                    apply_button_color(&mut styles.button_action, &property, value);
                }
                ".button-dismiss" => {
                    apply_button_color(&mut styles.button_dismiss, &property, value);
                }
                ".hint" => {
                    apply_hint_color(&mut styles.hint, &property, value);
                }
                _ => {}
            }
        }
    }

    styles
}

fn apply_notification_color(colors: &mut NotificationColors, property: &str, value: &str) {
    match property {
        "background-color" | "background" => {
            colors.background = parse_hex_color(value);
        }
        "border-color" => {
            colors.border_color = parse_hex_color(value);
        }
        _ => {}
    }
}

fn apply_text_color(colors: &mut TextColors, property: &str, value: &str) {
    match property {
        "color" => {
            colors.color = parse_hex_color(value);
        }
        "background-color" | "background" => {
            colors.background = parse_hex_color(value);
        }
        "border-color" => {
            colors.border_color = parse_hex_color(value);
        }
        _ => {}
    }
}

fn apply_progress_color(colors: &mut ProgressColors, property: &str, value: &str) {
    match property {
        "color" => {
            colors.color = parse_hex_color(value);
        }
        "background-color" | "background" => {
            colors.background = parse_hex_color(value);
        }
        "border-color" => {
            colors.border_color = parse_hex_color(value);
        }
        _ => {}
    }
}

fn apply_button_color(colors: &mut ButtonColors, property: &str, value: &str) {
    match property {
        "color" => {
            colors.color = parse_hex_color(value);
        }
        "background-color" | "background" => {
            colors.background = parse_hex_color(value);
        }
        "border-color" => {
            colors.border_color = parse_hex_color(value);
        }
        _ => {}
    }
}

fn apply_hint_color(colors: &mut HintColors, property: &str, value: &str) {
    match property {
        "color" => {
            colors.color = parse_hex_color(value);
        }
        "background-color" | "background" => {
            colors.background = parse_hex_color(value);
        }
        "border-color" => {
            colors.border_color = parse_hex_color(value);
        }
        _ => {}
    }
}

/// Parse hex color string into RGBA bytes
/// Supports formats: #RGB, #RGBA, #RRGGBB, #RRGGBBAA
fn parse_hex_color(value: &str) -> Option<[u8; 4]> {
    let value = value.trim();

    // Handle named colors
    match value.to_lowercase().as_str() {
        "black" => return Some([0, 0, 0, 255]),
        "white" => return Some([255, 255, 255, 255]),
        "red" => return Some([255, 0, 0, 255]),
        "green" => return Some([0, 128, 0, 255]),
        "blue" => return Some([0, 0, 255, 255]),
        "yellow" => return Some([255, 255, 0, 255]),
        "purple" => return Some([128, 0, 128, 255]),
        "cyan" => return Some([0, 255, 255, 255]),
        "magenta" => return Some([255, 0, 255, 255]),
        "transparent" => return Some([0, 0, 0, 0]),
        _ => {}
    }

    // Handle hex colors
    if let Some(hex) = value.strip_prefix('#') {
        match hex.len() {
            3 => {
                // #RGB
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                return Some([r * 17, g * 17, b * 17, 255]);
            }
            4 => {
                // #RGBA
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4], 16).ok()?;
                return Some([r * 17, g * 17, b * 17, a * 17]);
            }
            6 => {
                // #RRGGBB
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                return Some([r, g, b, 255]);
            }
            8 => {
                // #RRGGBBAA
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                return Some([r, g, b, a]);
            }
            _ => {}
        }
    }

    // Handle rgb() and rgba()
    if value.starts_with("rgb(") && value.ends_with(')') {
        let rgb = &value[4..value.len() - 1];
        let parts: Vec<&str> = rgb.split(',').map(str::trim).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            return Some([r, g, b, 255]);
        }
    } else if value.starts_with("rgba(") && value.ends_with(')') {
        let rgba = &value[5..value.len() - 1];
        let parts: Vec<&str> = rgba.split(',').map(str::trim).collect();
        if parts.len() == 4 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            let a = (parts[3].parse::<f32>().ok()? * 255.0) as u8;
            return Some([r, g, b, a]);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(parse_hex_color("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(parse_hex_color("#000"), Some([0, 0, 0, 255]));
        assert_eq!(parse_hex_color("#f00"), Some([255, 0, 0, 255]));
        assert_eq!(parse_hex_color("#ffffff"), Some([255, 255, 255, 255]));
        assert_eq!(parse_hex_color("#1a1b26"), Some([26, 27, 38, 255]));
        assert_eq!(parse_hex_color("#1a1b26ff"), Some([26, 27, 38, 255]));
        assert_eq!(parse_hex_color("#1a1b2680"), Some([26, 27, 38, 128]));
        assert_eq!(parse_hex_color("black"), Some([0, 0, 0, 255]));
        assert_eq!(parse_hex_color("white"), Some([255, 255, 255, 255]));
        assert_eq!(parse_hex_color("rgb(26, 27, 38)"), Some([26, 27, 38, 255]));
        assert_eq!(
            parse_hex_color("rgba(26, 27, 38, 0.5)"),
            Some([26, 27, 38, 127])
        );
    }

    #[test]
    fn test_parse_css() {
        let css = r#"
            .notification { background-color: #1a1b26; border-color: #7aa2f7; }
            .notification:hover { background-color: #24283b; }
            .summary { color: #c0caf5; }
        "#;

        let styles = parse_css(css);

        assert_eq!(styles.notification.background, Some([26, 27, 38, 255]));
        assert_eq!(styles.notification.border_color, Some([122, 162, 247, 255]));
        assert_eq!(
            styles.notification_hover.background,
            Some([36, 40, 59, 255])
        );
        assert_eq!(styles.summary.color, Some([192, 202, 245, 255]));
    }
}
