use super::{
    Text,
    markup::{Parser, Tag},
};
use crate::{
    Urgency,
    components::{self, Bounds, Component, Data},
    config,
    manager::UiState,
    utils::buffers,
};
use glyphon::{Attrs, Buffer, Color, Family, FontSystem, Shaping, Stretch, Style, Weight};
use std::sync::{Arc, atomic::Ordering};

#[derive(Debug)]
pub struct Anchor {
    pub href: Arc<str>,
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub bounds: Bounds,
}

impl Anchor {
    pub fn get_bounds(&self) -> Bounds {
        self.bounds.clone()
    }
}

pub struct Body {
    context: components::Context,
    pub anchors: Vec<Arc<Anchor>>,
    pub buffer: Buffer,
    x: f32,
    y: f32,
}

fn parse_color(value: &str) -> Option<glyphon::Color> {
    let value = value.trim();
    match value.to_lowercase().as_str() {
        "black" => return Some(glyphon::Color::rgba(0, 0, 0, 255)),
        "white" => return Some(glyphon::Color::rgba(255, 255, 255, 255)),
        "red" => return Some(glyphon::Color::rgba(255, 0, 0, 255)),
        "green" => return Some(glyphon::Color::rgba(0, 128, 0, 255)),
        "blue" => return Some(glyphon::Color::rgba(0, 0, 255, 255)),
        "yellow" => return Some(glyphon::Color::rgba(255, 255, 0, 255)),
        "purple" => return Some(glyphon::Color::rgba(128, 0, 128, 255)),
        "cyan" => return Some(glyphon::Color::rgba(0, 255, 255, 255)),
        "magenta" => return Some(glyphon::Color::rgba(255, 0, 255, 255)),
        _ => {}
    }

    if let Some(hex) = value.strip_prefix('#') {
        match hex.len() {
            3 => {
                if let (Some(r), Some(g), Some(b)) = (
                    u8::from_str_radix(&hex[0..1], 16).ok(),
                    u8::from_str_radix(&hex[1..2], 16).ok(),
                    u8::from_str_radix(&hex[2..3], 16).ok(),
                ) {
                    return Some(glyphon::Color::rgba(r * 17, g * 17, b * 17, 255));
                }
            }
            4 => {
                if let (Some(r), Some(g), Some(b), Some(a)) = (
                    u8::from_str_radix(&hex[0..1], 16).ok(),
                    u8::from_str_radix(&hex[1..2], 16).ok(),
                    u8::from_str_radix(&hex[2..3], 16).ok(),
                    u8::from_str_radix(&hex[3..4], 16).ok(),
                ) {
                    return Some(glyphon::Color::rgba(r * 17, g * 17, b * 17, a * 17));
                }
            }
            6 => {
                if let (Some(r), Some(g), Some(b)) = (
                    u8::from_str_radix(&hex[0..2], 16).ok(),
                    u8::from_str_radix(&hex[2..4], 16).ok(),
                    u8::from_str_radix(&hex[4..6], 16).ok(),
                ) {
                    return Some(glyphon::Color::rgba(r, g, b, 255));
                }
            }
            8 => {
                if let (Some(r), Some(g), Some(b), Some(a)) = (
                    u8::from_str_radix(&hex[0..2], 16).ok(),
                    u8::from_str_radix(&hex[2..4], 16).ok(),
                    u8::from_str_radix(&hex[4..6], 16).ok(),
                    u8::from_str_radix(&hex[6..8], 16).ok(),
                ) {
                    return Some(glyphon::Color::rgba(r, g, b, a));
                }
            }
            _ => {}
        }
    }

    if value.starts_with("rgb(") && value.ends_with(')') {
        let rgb = &value[4..value.len() - 1];
        let parts: Vec<&str> = rgb.split(',').map(|s| s.trim()).collect();
        if parts.len() == 3 {
            if let (Some(r), Some(g), Some(b)) = (
                parts[0].parse::<u8>().ok(),
                parts[1].parse::<u8>().ok(),
                parts[2].parse::<u8>().ok(),
            ) {
                return Some(glyphon::Color::rgba(r, g, b, 255));
            }
        }
    } else if value.starts_with("rgba(") && value.ends_with(')') {
        let rgba = &value[5..value.len() - 1];
        let parts: Vec<&str> = rgba.split(',').map(|s| s.trim()).collect();
        if parts.len() == 4 {
            if let (Some(r), Some(g), Some(b), Some(a)) = (
                parts[0].parse::<u8>().ok(),
                parts[1].parse::<u8>().ok(),
                parts[2].parse::<u8>().ok(),
                (parts[3].parse::<f32>().ok().map(|a| (a * 255.0) as u8)),
            ) {
                return Some(glyphon::Color::rgba(r, g, b, a));
            }
        }
    }

    None
}

impl Text for Body {
    fn set_size(&mut self, font_system: &mut FontSystem, width: Option<f32>, height: Option<f32>) {
        self.buffer.set_size(font_system, width, height);
    }

    fn set_text<T>(&mut self, font_system: &mut FontSystem, text: T)
    where
        T: AsRef<str>,
    {
        let family = Arc::clone(&self.get_style().family);

        let attrs = Attrs::new()
            .metadata(0.7_f32.to_bits() as usize)
            .family(glyphon::Family::Name(&family));

        let mut anchors = Vec::new();

        let mut parser = Parser::new(text.as_ref().to_string());
        let body = parser.parse();
        let spans = body
            .iter()
            .map(|tag| match tag {
                Tag::Bold(text) => (text.as_str(), attrs.clone().weight(Weight::BOLD)),
                Tag::Italic(text) => (text.as_str(), attrs.clone().style(Style::Italic)),
                Tag::Underline(text) => (text.as_str(), attrs.clone()),
                Tag::Image { alt, src: _ } => (alt.as_str(), attrs.clone()),
                Tag::Anchor {
                    href,
                    text,
                    position,
                } => {
                    let anchor = Anchor {
                        href: href.as_str().into(),
                        line: position.line,
                        start: position.column,
                        end: position.column + text.len() - 1,
                        bounds: Bounds::default(),
                    };
                    anchors.push(anchor);
                    (text.as_str(), attrs.clone().color(Color::rgb(0, 0, 255)))
                }
                Tag::Span { text, attributes } => {
                    let mut attrs = attrs.clone();
                    attributes
                        .iter()
                        .for_each(|(key, value)| match key.as_str() {
                            "font_desc" => {}
                            "font_family" | "face" => {
                                attrs = attrs.clone().family(Family::Name(value))
                            }

                            "font_size" | "size" => {
                                if let Ok(value) = value.parse::<f32>() {
                                    let dpi = 96.0;
                                    let font_size = value * dpi / 72.0;
                                    attrs = attrs
                                        .clone()
                                        .metrics(glyphon::Metrics::new(font_size, font_size * 1.2))
                                }
                            }
                            "letter_spacing" => {
                                if let Ok(spacing) = value.parse::<f32>() {
                                    attrs = attrs.clone().letter_spacing(spacing)
                                }
                            }
                            "rise" | "baseline_shift" => {}
                            "line_height" => {
                                if let Ok(height) = value.parse::<f32>() {
                                    let current_metrics = self.buffer.metrics();
                                    attrs = attrs.clone().metrics(glyphon::Metrics::new(
                                        current_metrics.font_size,
                                        height,
                                    ))
                                }
                            }

                            "font_style" | "style" => match value.as_ref() {
                                "normal" => attrs = attrs.clone().style(Style::Normal),
                                "oblique" => attrs = attrs.clone().style(Style::Oblique),
                                "italic" => attrs = attrs.clone().style(Style::Italic),
                                _ => {}
                            },
                            "font_weight" | "weight" => {
                                if value == "bold" {
                                    attrs = attrs.clone().weight(Weight::BOLD)
                                } else if value == "normal" {
                                    attrs = attrs.clone().weight(Weight::NORMAL)
                                } else if let Ok(weight_value) = value.parse::<u16>() {
                                    let weight = match weight_value {
                                        100 => Weight::THIN,
                                        200 => Weight::EXTRA_LIGHT,
                                        300 => Weight::LIGHT,
                                        400 => Weight::NORMAL,
                                        500 => Weight::MEDIUM,
                                        600 => Weight::SEMIBOLD,
                                        700 => Weight::BOLD,
                                        800 => Weight::EXTRA_BOLD,
                                        900 => Weight::BLACK,
                                        _ => Weight::NORMAL,
                                    };
                                    attrs = attrs.clone().weight(weight)
                                }
                            }
                            "font_variant" | "variant" => match value.as_ref() {
                                "normal" => {}
                                "small-caps" => {}
                                _ => {}
                            },
                            "font_stretch" | "stretch" => match value.as_ref() {
                                "ultra-condensed" => {
                                    attrs = attrs.clone().stretch(Stretch::UltraCondensed)
                                }
                                "extra-condensed" => {
                                    attrs = attrs.clone().stretch(Stretch::ExtraCondensed)
                                }
                                "condensed" => attrs = attrs.clone().stretch(Stretch::Condensed),
                                "semi-condensed" => {
                                    attrs = attrs.clone().stretch(Stretch::SemiCondensed)
                                }
                                "normal" => attrs = attrs.clone().stretch(Stretch::Normal),
                                "semi-expanded" => {
                                    attrs = attrs.clone().stretch(Stretch::SemiExpanded)
                                }
                                "expanded" => attrs = attrs.clone().stretch(Stretch::Expanded),
                                "extra-expanded" => {
                                    attrs = attrs.clone().stretch(Stretch::ExtraExpanded)
                                }
                                "ultra-expanded" => {
                                    attrs = attrs.clone().stretch(Stretch::UltraExpanded)
                                }
                                _ => {}
                            },
                            "text_transform" => match value.as_ref() {
                                "none" => {}
                                "lowercase" => {}
                                "uppercase" => {}
                                "capitalize" => {}
                                _ => {}
                            },

                            "font_features" => {
                                // OpenType font features
                                // attrs = attrs.clone().font_features(value)
                            }
                            "font_variations" => {}

                            "foreground" | "fgcolor" | "color" => {
                                if let Some(color) = parse_color(value) {
                                    attrs = attrs.clone().color(color)
                                }
                            }
                            "background" | "bgcolor" => {}
                            "alpha" => {}
                            "foreground_alpha" | "fgalpha" => {}
                            "background_alpha" | "bgalpha" => {}

                            "strikethrough" => {}
                            "strikethrough_color" => {}
                            "underline" => {}
                            "underline_color" => {}
                            "overline" => {}
                            "overline_color" => {}

                            "gravity" => match value.as_ref() {
                                "south" => {}
                                "east" => {}
                                "north" => {}
                                "west" => {}
                                "auto" => {}
                                _ => {}
                            },
                            "gravity_hint" => match value.as_ref() {
                                "natural" => {}
                                "strong" => {}
                                "line" => {}
                                _ => {}
                            },
                            "fallback" => {}
                            "lang" => {}
                            "insert_hyphens" | "allow_breaks" | "insert" | "allow" => {}
                            "wrap" => {}
                            "show" => {}

                            _ => {}
                        });
                    (text.as_str(), attrs.clone())
                }
                Tag::Text(text) => (text.as_str(), attrs.clone()),
            })
            .collect::<Vec<_>>();

        self.buffer
            .set_rich_text(font_system, spans, &attrs, Shaping::Advanced, None);

        anchors.iter_mut().for_each(|anchor| {
            if let Some(line) = self.buffer.layout_runs().nth(anchor.line) {
                let first = line.glyphs.get(anchor.start);
                let last = line.glyphs.get(anchor.end);

                if let (Some(first), Some(last)) = (first, last) {
                    anchor.bounds = Bounds {
                        x: first.x,
                        y: line.line_top,
                        width: last.x + last.w,
                        height: line.line_height,
                    };
                }
            };
        });

        self.anchors = anchors.into_iter().map(Arc::new).collect();
    }
}

impl Component for Body {
    type Style = config::text::Body;

    fn get_config(&self) -> &crate::config::Config {
        &self.context.config
    }

    fn get_app_name(&self) -> &str {
        &self.context.app_name
    }

    fn get_id(&self) -> u32 {
        self.context.id
    }

    fn get_ui_state(&self) -> &UiState {
        &self.context.ui_state
    }

    fn get_style(&self) -> &Self::Style {
        &self.get_notification_style().body
    }

    fn get_instances(&self, urgency: &Urgency) -> Vec<buffers::Instance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds();

        vec![buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: self.context.ui_state.scale.load(Ordering::Relaxed),
            depth: 0.8,
        }]
    }

    fn get_text_areas(&self, urgency: &crate::Urgency) -> Vec<glyphon::TextArea<'_>> {
        let style = self.get_style();
        let render_bounds = self.get_render_bounds();

        let content_width = render_bounds.width
            - style.border.size.left
            - style.border.size.right
            - style.padding.left
            - style.padding.right;

        let content_height = render_bounds.height
            - style.border.size.top
            - style.border.size.bottom
            - style.padding.top
            - style.padding.bottom;

        let left = render_bounds.x + style.border.size.left + style.padding.left;
        let top = render_bounds.y + style.border.size.top + style.padding.top;

        vec![glyphon::TextArea {
            buffer: &self.buffer,
            left,
            top,
            scale: self.context.ui_state.scale.load(Ordering::Relaxed),
            bounds: glyphon::TextBounds {
                left: left as i32,
                top: top as i32,
                right: (left + content_width) as i32,
                bottom: (top + content_height) as i32,
            },
            default_color: style.color.into_glyphon(urgency),
            custom_glyphs: &[],
        }]
    }

    fn get_textures(&self) -> Vec<crate::rendering::texture_renderer::TextureArea<'_>> {
        Vec::new()
    }

    fn get_bounds(&self) -> Bounds {
        let style = self.get_style();
        let (width, total_lines) = self
            .buffer
            .layout_runs()
            .fold((0.0, 0.0), |(width, total_lines), run| {
                (run.line_w.max(width), total_lines + 1.0)
            });

        Bounds {
            x: self.x,
            y: self.y,
            width: width
                + style.margin.left
                + style.margin.right
                + style.padding.left
                + style.padding.right
                + style.border.size.left
                + style.border.size.right,
            height: total_lines * self.buffer.metrics().line_height
                + style.margin.top
                + style.margin.bottom
                + style.padding.top
                + style.padding.bottom
                + style.border.size.top
                + style.border.size.bottom,
        }
    }

    fn get_render_bounds(&self) -> Bounds {
        let style = self.get_style();
        let bounds = self.get_bounds();
        Bounds {
            x: bounds.x + style.margin.left,
            y: bounds.y + style.margin.top,
            width: bounds.width - style.margin.left - style.margin.right,
            height: bounds.height - style.margin.top - style.margin.bottom,
        }
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn get_data(&self, urgency: &Urgency) -> Vec<Data<'_>> {
        self.get_instances(urgency)
            .into_iter()
            .map(Data::Instance)
            .chain(self.get_text_areas(urgency).into_iter().map(Data::TextArea))
            .collect()
    }
}

impl Body {
    pub fn new(context: components::Context, font_system: &mut FontSystem) -> Self {
        let dpi = 96.0;
        let font_size = context.config.styles.default.font.size * dpi / 72.0;
        let mut buffer = Buffer::new(
            font_system,
            glyphon::Metrics::new(font_size, font_size * 1.2),
        );
        buffer.shape_until_scroll(font_system, true);

        Self {
            context,
            buffer,
            x: 0.,
            y: 0.,
            anchors: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        components::text::{
            Text,
            markup::{Parser, Tag},
        },
        config::Config,
        manager::UiState,
    };
    use glyphon::{Color, FontSystem};

    #[test]
    fn test_body() {
        let mut font_system = FontSystem::new();

        let context = components::Context {
            id: 0,
            app_name: "".into(),
            config: Arc::new(Config::default()),
            ui_state: UiState::default(),
        };
        let mut body = Body::new(context, &mut font_system);

        body.set_text(
            &mut font_system,
            "Hello world\n<b>Hello world</b>\n<i>Hello world</i>\n<a href=\"\">Hello world</a>\n<img alt=\"Hello world\" href=\"/tmp/image.png\">",
        );

        let lines = body.buffer.lines;
        assert_eq!(lines.first().unwrap().text(), "Hello world");
        assert_eq!(lines.get(1).unwrap().text(), "Hello world");
        assert_eq!(lines.get(2).unwrap().text(), "Hello world");
        assert_eq!(lines.get(3).unwrap().text(), "Hello world");
        assert_eq!(lines.get(4).unwrap().text(), "Hello world");
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn test_plain_url_detection() {
        let mut font_system = FontSystem::new();
        let context = components::Context {
            id: 0,
            app_name: "".into(),
            config: Arc::new(Config::default()),
            ui_state: UiState::default(),
        };
        let mut body = Body::new(context, &mut font_system);

        body.set_text(
            &mut font_system,
            "Check this website: https://example.com for more info.",
        );

        assert_eq!(body.anchors.len(), 1);
        assert_eq!(body.anchors[0].href.as_ref(), "https://example.com");
        assert_eq!(body.anchors[0].line, 0);
    }

    #[test]
    fn test_multiple_urls() {
        let mut font_system = FontSystem::new();
        let context = components::Context {
            id: 0,
            app_name: "".into(),
            config: Arc::new(Config::default()),
            ui_state: UiState::default(),
        };
        let mut body = Body::new(context, &mut font_system);

        body.set_text(
            &mut font_system,
            "First URL: https://example.com and second URL: http://test.org!",
        );

        assert_eq!(body.anchors.len(), 2);
        assert_eq!(body.anchors[0].href.as_ref(), "https://example.com");
        assert_eq!(body.anchors[1].href.as_ref(), "http://test.org");
    }

    #[test]
    fn test_span_parser_basics() {
        let mut parser = Parser::new(String::from("<span color=\"red\">Red text</span>"));
        let tags = parser.parse();

        assert_eq!(tags.len(), 1);
        match &tags[0] {
            Tag::Span { text, attributes } => {
                assert_eq!(text, "Red text");
                assert_eq!(attributes.len(), 1);
                assert_eq!(attributes.get("color").unwrap(), "red");
            }
            _ => panic!("Expected Tag::Span"),
        }
    }

    #[test]
    fn test_span_parser_multiple_attributes() {
        let mut parser = Parser::new(String::from(
            "<span color=\"blue\" weight=\"bold\" style=\"italic\">Formatted text</span>",
        ));
        let tags = parser.parse();

        assert_eq!(tags.len(), 1);
        match &tags[0] {
            Tag::Span { text, attributes } => {
                assert_eq!(text, "Formatted text");
                assert_eq!(attributes.len(), 3);
                assert_eq!(attributes.get("color").unwrap(), "blue");
                assert_eq!(attributes.get("weight").unwrap(), "bold");
                assert_eq!(attributes.get("style").unwrap(), "italic");
            }
            _ => panic!("Expected Tag::Span"),
        }
    }

    #[test]
    fn test_mixed_tags() {
        let mut parser = Parser::new(String::from(
            "Normal <b>bold</b><span color=\"red\">red</span> text",
        ));
        let tags = parser.parse();

        assert_eq!(tags.len(), 4);
        match &tags[0] {
            Tag::Text(text) => assert_eq!(text, "Normal "),
            _ => panic!("Expected Tag::Text"),
        }
        match &tags[1] {
            Tag::Bold(text) => assert_eq!(text, "bold"),
            _ => panic!("Expected Tag::Bold"),
        }
        match &tags[2] {
            Tag::Span { text, attributes } => {
                assert_eq!(text, "red");
                assert_eq!(attributes.get("color").unwrap(), "red");
            }
            _ => panic!("Expected Tag::Span"),
        }
        match &tags[3] {
            Tag::Text(text) => assert_eq!(text, " text"),
            _ => panic!("Expected Tag::Text"),
        }
    }

    #[test]
    fn test_named_colors() {
        assert_eq!(parse_color("red"), Some(Color::rgba(255, 0, 0, 255)));
        assert_eq!(parse_color("green"), Some(Color::rgba(0, 128, 0, 255)));
        assert_eq!(parse_color("blue"), Some(Color::rgba(0, 0, 255, 255)));
        assert_eq!(parse_color("black"), Some(Color::rgba(0, 0, 0, 255)));
        assert_eq!(parse_color("white"), Some(Color::rgba(255, 255, 255, 255)));

        assert_eq!(parse_color("Red"), Some(Color::rgba(255, 0, 0, 255)));
        assert_eq!(parse_color("BLUE"), Some(Color::rgba(0, 0, 255, 255)));
    }

    #[test]
    fn test_invalid_named_color() {
        assert_eq!(parse_color("notacolor"), None);
    }

    #[test]
    fn test_hex_colors() {
        assert_eq!(parse_color("#f00"), Some(Color::rgba(255, 0, 0, 255)));

        assert_eq!(parse_color("#f008"), Some(Color::rgba(255, 0, 0, 136)));

        assert_eq!(parse_color("#ff0000"), Some(Color::rgba(255, 0, 0, 255)));

        assert_eq!(parse_color("#ff000080"), Some(Color::rgba(255, 0, 0, 128)));
    }

    #[test]
    fn test_invalid_hex_colors() {
        assert!(parse_color("#f0").is_none());
        assert!(parse_color("#xyz").is_none());
    }

    #[test]
    fn test_rgb_colors() {
        assert_eq!(
            parse_color("rgb(255, 0, 0)"),
            Some(Color::rgba(255, 0, 0, 255))
        );
        assert_eq!(
            parse_color("rgb(0, 255, 0)"),
            Some(Color::rgba(0, 255, 0, 255))
        );
        assert_eq!(
            parse_color("rgb(0, 0, 255)"),
            Some(Color::rgba(0, 0, 255, 255))
        );
    }

    #[test]
    fn test_rgba_colors() {
        assert_eq!(
            parse_color("rgba(255, 0, 0, 1.0)"),
            Some(Color::rgba(255, 0, 0, 255))
        );
        assert_eq!(
            parse_color("rgba(255, 0, 0, 0.5)"),
            Some(Color::rgba(255, 0, 0, 127))
        );
        assert_eq!(
            parse_color("rgba(255, 0, 0, 0.0)"),
            Some(Color::rgba(255, 0, 0, 0))
        );
    }

    #[test]
    fn test_invalid_rgb_rgba() {
        assert!(parse_color("rgb(255, 0)").is_none());
        assert!(parse_color("rgba(255, 0, 0)").is_none());

        assert!(parse_color("rgb(300, 0, 0)").is_none());
    }

    #[test]
    fn test_color_parsing_edge_cases() {
        assert_eq!(
            parse_color("  rgb(255, 0, 0)  "),
            Some(Color::rgba(255, 0, 0, 255))
        );

        assert_eq!(
            parse_color("rgb(255,0,0)"),
            Some(Color::rgba(255, 0, 0, 255))
        );
    }

    #[test]
    fn test_attribute_inheritance() {
        let mut font_system = FontSystem::new();
        let context = components::Context {
            id: 0,
            app_name: "".into(),
            config: Arc::new(Config::default()),
            ui_state: UiState::default(),
        };
        let mut body = Body::new(context, &mut font_system);

        body.set_text(
            &mut font_system,
            "<span color=\"red\" weight=\"bold\" style=\"italic\" font_size=\"18\">Multi-styled text</span>",
        );

        assert_eq!(body.buffer.lines.len(), 1);
        assert_eq!(body.buffer.lines[0].text(), "Multi-styled text");
    }

    #[test]
    fn test_bounds_calculation() {
        let mut font_system = FontSystem::new();
        let context = components::Context {
            id: 0,
            app_name: "".into(),
            config: Arc::new(Config::default()),
            ui_state: UiState::default(),
        };
        let mut body = Body::new(context, &mut font_system);

        body.set_position(10.0, 20.0);
        body.set_text(
            &mut font_system,
            "Line 1\n<span color=\"blue\">Line 2</span>\n<span weight=\"bold\">Line 3</span>",
        );

        let bounds = body.get_bounds();
        assert_eq!(bounds.x, 10.0);
        assert_eq!(bounds.y, 20.0);
        assert!(bounds.width > 0.0);
        assert!(bounds.height > 0.0);
    }

    #[test]
    fn test_render_data() {
        let mut font_system = FontSystem::new();
        let context = components::Context {
            id: 0,
            app_name: "".into(),
            config: Arc::new(Config::default()),
            ui_state: UiState::default(),
        };
        let mut body = Body::new(context, &mut font_system);

        body.set_position(10.0, 20.0);
        body.set_text(&mut font_system, "<span color=\"blue\">Blue text</span>");

        let data = body.get_data(&Urgency::Normal);

        assert!(data.len() >= 2);

        let mut has_instance = false;
        let mut has_text_area = false;

        for item in data {
            match item {
                Data::Instance(_) => has_instance = true,
                Data::TextArea(_) => has_text_area = true,
                _ => {}
            }
        }

        assert!(has_instance);
        assert!(has_text_area);
    }
}
