use crate::styles::{
    BorderRadius, ButtonState, Color, Hint, NotificationCounter, Progress, StyleState, Styles,
    TextStyle,
};
use simplecss::{Declaration, StyleSheet};

/// Parse a color value from CSS (hex, rgb, rgba formats)
fn parse_color_value(value: &str) -> Option<[u8; 4]> {
    let value = value.trim();

    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex_color(hex);
    }

    if let Some(rgb) = value.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = rgb.split(',').map(str::trim).collect();
        if parts.len() == 3 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                parts[0].parse::<u8>(),
                parts[1].parse::<u8>(),
                parts[2].parse::<u8>(),
            ) {
                return Some([r, g, b, 255]);
            }
        }
    }

    if let Some(rgba) = value
        .strip_prefix("rgba(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = rgba.split(',').map(str::trim).collect();
        if parts.len() == 4 {
            if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
                parts[0].parse::<u8>(),
                parts[1].parse::<u8>(),
                parts[2].parse::<u8>(),
                parts[3].parse::<f32>(),
            ) {
                return Some([r, g, b, (a * 255.0) as u8]);
            }
        }
    }

    match value.to_lowercase().as_str() {
        "black" => Some([0, 0, 0, 255]),
        "white" => Some([255, 255, 255, 255]),
        "red" => Some([255, 0, 0, 255]),
        "green" => Some([0, 128, 0, 255]),
        "blue" => Some([0, 0, 255, 255]),
        "yellow" => Some([255, 255, 0, 255]),
        "transparent" => Some([0, 0, 0, 0]),
        _ => None,
    }
}

fn parse_hex_color(hex: &str) -> Option<[u8; 4]> {
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some([r, g, b, 255])
        }
        4 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            let a = u8::from_str_radix(&hex[3..4], 16).ok()? * 17;
            Some([r, g, b, a])
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some([r, g, b, 255])
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some([r, g, b, a])
        }
        _ => None,
    }
}

fn parse_border_radius(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(px) = value.strip_suffix("px") {
        return px.trim().parse().ok();
    }
    value.parse().ok()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Urgency {
    All,
    Low,
    Normal,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    All,
    Default,
    Focused,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Element {
    Notification,
    Summary,
    Body,
    Button,
    ButtonAction,
    ButtonDismiss,
    Progress,
    Hint,
    Counter,
    Icon,
}

struct SelectorMatch {
    element: Element,
    urgency: Urgency,
    state: State,
}

fn parse_selector(selector_str: &str) -> Option<SelectorMatch> {
    let selector_str = selector_str.trim();

    let element = if selector_str.contains(".notification") || selector_str == "*" {
        Element::Notification
    } else if selector_str.contains(".summary") {
        Element::Summary
    } else if selector_str.contains(".body") {
        Element::Body
    } else if selector_str.contains(".button.action") {
        Element::ButtonAction
    } else if selector_str.contains(".button.dismiss") {
        Element::ButtonDismiss
    } else if selector_str.contains(".button") {
        Element::Button
    } else if selector_str.contains(".progress") {
        Element::Progress
    } else if selector_str.contains(".hint") {
        Element::Hint
    } else if selector_str.contains(".counter") {
        Element::Counter
    } else if selector_str.contains(".icon") {
        Element::Icon
    } else {
        return None;
    };

    let urgency = if selector_str.contains(".low") {
        Urgency::Low
    } else if selector_str.contains(".normal") {
        Urgency::Normal
    } else if selector_str.contains(".critical") {
        Urgency::Critical
    } else {
        Urgency::All
    };

    let state = if selector_str.contains(":hover") || selector_str.contains(".focused") {
        State::Focused
    } else if selector_str.contains(".unfocused") || selector_str.contains(".default") {
        State::Default
    } else {
        State::All
    };

    Some(SelectorMatch {
        element,
        urgency,
        state,
    })
}

fn apply_color_to_urgency(color: &mut Color, value: [u8; 4], urgency: Urgency) {
    match urgency {
        Urgency::All => {
            color.urgency_low = value;
            color.urgency_normal = value;
            color.urgency_critical = value;
        }
        Urgency::Low => color.urgency_low = value,
        Urgency::Normal => color.urgency_normal = value,
        Urgency::Critical => color.urgency_critical = value,
    }
}

fn apply_declarations_to_style_state(
    style: &mut StyleState,
    declarations: &[Declaration<'_>],
    urgency: Urgency,
) {
    for decl in declarations {
        match decl.name {
            "background" | "background-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.background, color, urgency);
                }
            }
            "border-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.border.color, color, urgency);
                }
            }
            "border-radius" => {
                if let Some(radius) = parse_border_radius(decl.value) {
                    style.border.radius = BorderRadius {
                        top_left: radius,
                        top_right: radius,
                        bottom_left: radius,
                        bottom_right: radius,
                    };
                }
            }
            "color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.font.color, color, urgency);
                }
            }
            _ => {}
        }
    }
}

fn apply_declarations_to_text_style(
    style: &mut TextStyle,
    declarations: &[Declaration<'_>],
    urgency: Urgency,
) {
    for decl in declarations {
        match decl.name {
            "background" | "background-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.background, color, urgency);
                }
            }
            "border-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.border.color, color, urgency);
                }
            }
            "border-radius" => {
                if let Some(radius) = parse_border_radius(decl.value) {
                    style.border.radius = BorderRadius {
                        top_left: radius,
                        top_right: radius,
                        bottom_left: radius,
                        bottom_right: radius,
                    };
                }
            }
            "color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.color, color, urgency);
                }
            }
            _ => {}
        }
    }
}

fn apply_declarations_to_button_state(
    style: &mut ButtonState,
    declarations: &[Declaration<'_>],
    urgency: Urgency,
) {
    for decl in declarations {
        match decl.name {
            "background" | "background-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.background, color, urgency);
                }
            }
            "border-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.border.color, color, urgency);
                }
            }
            "border-radius" => {
                if let Some(radius) = parse_border_radius(decl.value) {
                    style.border.radius = BorderRadius {
                        top_left: radius,
                        top_right: radius,
                        bottom_left: radius,
                        bottom_right: radius,
                    };
                }
            }
            "color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.font.color, color, urgency);
                }
            }
            _ => {}
        }
    }
}

fn apply_declarations_to_progress(
    style: &mut Progress,
    declarations: &[Declaration<'_>],
    urgency: Urgency,
) {
    for decl in declarations {
        match decl.name {
            "background" | "background-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.incomplete_color, color, urgency);
                }
            }
            "color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.complete_color, color, urgency);
                }
            }
            "border-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.border.color, color, urgency);
                }
            }
            "border-radius" => {
                if let Some(radius) = parse_border_radius(decl.value) {
                    style.border.radius = BorderRadius {
                        top_left: radius,
                        top_right: radius,
                        bottom_left: radius,
                        bottom_right: radius,
                    };
                }
            }
            _ => {}
        }
    }
}

fn apply_declarations_to_hint(
    style: &mut Hint,
    declarations: &[Declaration<'_>],
    urgency: Urgency,
) {
    for decl in declarations {
        match decl.name {
            "background" | "background-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.background, color, urgency);
                }
            }
            "border-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.border.color, color, urgency);
                }
            }
            "border-radius" => {
                if let Some(radius) = parse_border_radius(decl.value) {
                    style.border.radius = BorderRadius {
                        top_left: radius,
                        top_right: radius,
                        bottom_left: radius,
                        bottom_right: radius,
                    };
                }
            }
            "color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.font.color, color, urgency);
                }
            }
            _ => {}
        }
    }
}

fn apply_declarations_to_counter(
    style: &mut NotificationCounter,
    declarations: &[Declaration<'_>],
    urgency: Urgency,
) {
    for decl in declarations {
        match decl.name {
            "background" | "background-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.background, color, urgency);
                }
            }
            "border-color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.border.color, color, urgency);
                }
            }
            "border-radius" => {
                if let Some(radius) = parse_border_radius(decl.value) {
                    style.border.radius = BorderRadius {
                        top_left: radius,
                        top_right: radius,
                        bottom_left: radius,
                        bottom_right: radius,
                    };
                }
            }
            "color" => {
                if let Some(color) = parse_color_value(decl.value) {
                    apply_color_to_urgency(&mut style.font.color, color, urgency);
                }
            }
            _ => {}
        }
    }
}

fn apply_to_urgency_styles(
    styles: &mut Styles,
    selector: &SelectorMatch,
    declarations: &[Declaration<'_>],
) {
    let urgencies: Vec<Urgency> = match selector.urgency {
        Urgency::All => vec![Urgency::Low, Urgency::Normal, Urgency::Critical],
        u => vec![u],
    };

    let states: Vec<State> = match selector.state {
        State::All => vec![State::Default, State::Focused],
        s => vec![s],
    };

    for urgency in &urgencies {
        let urgency_styles = match urgency {
            Urgency::Low => &mut styles.urgency_low,
            Urgency::Normal => &mut styles.urgency_normal,
            Urgency::Critical => &mut styles.urgency_critical,
            Urgency::All => continue,
        };

        for state in &states {
            let style_state = match state {
                State::Default => &mut urgency_styles.unfocused,
                State::Focused => &mut urgency_styles.focused,
                State::All => continue,
            };

            match selector.element {
                Element::Notification => {
                    apply_declarations_to_style_state(style_state, declarations, *urgency);
                }
                Element::Summary => {
                    apply_declarations_to_text_style(
                        &mut style_state.summary,
                        declarations,
                        *urgency,
                    );
                }
                Element::Body => {
                    apply_declarations_to_text_style(&mut style_state.body, declarations, *urgency);
                }
                Element::Button => match state {
                    State::Focused => {
                        apply_declarations_to_button_state(
                            &mut style_state.buttons.action.hover,
                            declarations,
                            *urgency,
                        );
                        apply_declarations_to_button_state(
                            &mut style_state.buttons.dismiss.hover,
                            declarations,
                            *urgency,
                        );
                    }
                    _ => {
                        apply_declarations_to_button_state(
                            &mut style_state.buttons.action.default,
                            declarations,
                            *urgency,
                        );
                        apply_declarations_to_button_state(
                            &mut style_state.buttons.dismiss.default,
                            declarations,
                            *urgency,
                        );
                    }
                },
                Element::ButtonAction => match state {
                    State::Focused => {
                        apply_declarations_to_button_state(
                            &mut style_state.buttons.action.hover,
                            declarations,
                            *urgency,
                        );
                    }
                    _ => {
                        apply_declarations_to_button_state(
                            &mut style_state.buttons.action.default,
                            declarations,
                            *urgency,
                        );
                    }
                },
                Element::ButtonDismiss => match state {
                    State::Focused => {
                        apply_declarations_to_button_state(
                            &mut style_state.buttons.dismiss.hover,
                            declarations,
                            *urgency,
                        );
                    }
                    _ => {
                        apply_declarations_to_button_state(
                            &mut style_state.buttons.dismiss.default,
                            declarations,
                            *urgency,
                        );
                    }
                },
                Element::Progress => {
                    apply_declarations_to_progress(
                        &mut style_state.progress,
                        declarations,
                        *urgency,
                    );
                }
                Element::Hint => {
                    apply_declarations_to_hint(&mut style_state.hint, declarations, *urgency);
                }
                Element::Icon => {
                    for decl in declarations {
                        if decl.name == "border-radius" {
                            if let Some(radius) = parse_border_radius(decl.value) {
                                style_state.icon.border.radius = BorderRadius {
                                    top_left: radius,
                                    top_right: radius,
                                    bottom_left: radius,
                                    bottom_right: radius,
                                };
                            }
                        }
                    }
                }
                Element::Counter => {}
            }
        }
    }

    if selector.element == Element::Counter {
        apply_declarations_to_counter(&mut styles.next, declarations, Urgency::All);
        apply_declarations_to_counter(&mut styles.prev, declarations, Urgency::All);
    }
}

pub fn parse_css(css: &str) -> Styles {
    let mut styles = Styles::default();

    if css.is_empty() {
        return styles;
    }

    let stylesheet = StyleSheet::parse(css);

    for rule in &stylesheet.rules {
        let selector_str = rule.selector.to_string();

        if let Some(selector) = parse_selector(&selector_str) {
            apply_to_urgency_styles(&mut styles, &selector, &rule.declarations);
        }
    }

    styles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(parse_color_value("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(parse_color_value("#000"), Some([0, 0, 0, 255]));
        assert_eq!(parse_color_value("#ff0000"), Some([255, 0, 0, 255]));
        assert_eq!(parse_color_value("#00ff00ff"), Some([0, 255, 0, 255]));
    }

    #[test]
    fn test_parse_rgb_color() {
        assert_eq!(parse_color_value("rgb(255, 0, 0)"), Some([255, 0, 0, 255]));
        assert_eq!(
            parse_color_value("rgba(0, 255, 0, 0.5)"),
            Some([0, 255, 0, 127])
        );
    }

    #[test]
    fn test_parse_css_notification() {
        let css = r#"
            .notification {
                background-color: #1a1b26;
                border-color: #a6e3a1;
            }
            .notification.urgency-critical {
                background-color: #ff0000;
            }
            .notification:hover {
                background-color: #2f3549;
            }
        "#;

        let styles = parse_css(css);

        assert_eq!(
            styles
                .urgency_critical
                .unfocused
                .background
                .urgency_critical,
            [255, 0, 0, 255]
        );
    }
}
