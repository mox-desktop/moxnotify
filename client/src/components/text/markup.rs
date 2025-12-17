use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

#[derive(Debug, PartialEq)]
pub enum Tag {
    Bold(String),
    Italic(String),
    Underline(String),
    Image {
        alt: String,
        src: String,
    },
    Anchor {
        href: String,
        text: String,
        position: Position,
    },
    Text(String),
    Span {
        text: String,
        attributes: HashMap<String, String>,
    },
}

pub struct Parser {
    pos: usize,
    line: usize,
    column: usize,
    text_column: usize,
    input: String,
}

impl Parser {
    #[must_use]
    pub fn new(input: String) -> Self {
        Parser {
            pos: 0,
            line: 0,
            column: 0,
            text_column: 0,
            input,
        }
    }

    pub fn parse(&mut self) -> Vec<Tag> {
        let mut result = Vec::new();
        while self.pos < self.input.len() {
            if self.input[self.pos..].starts_with('<') {
                if self.input[self.pos..].starts_with("<b>") {
                    self.consume_str("<b>", false);
                    let content = self.parse_until("</b>");
                    self.consume_str("</b>", false);
                    result.push(Tag::Bold(content));
                } else if self.input[self.pos..].starts_with("<i>") {
                    self.consume_str("<i>", false);
                    let content = self.parse_until("</i>");
                    self.consume_str("</i>", false);
                    result.push(Tag::Italic(content));
                } else if self.input[self.pos..].starts_with("<u>") {
                    self.consume_str("<u>", false);
                    let content = self.parse_until("</u>");
                    self.consume_str("</u>", false);
                    result.push(Tag::Underline(content));
                } else if self.input[self.pos..].starts_with("<img") {
                    let (tag_end, attributes) = self.parse_tag_and_attributes(false);
                    self.pos = tag_end + 1;
                    self.update_position_for_tag_end(false);
                    result.push(Tag::Image {
                        alt: attributes.get("alt").cloned().unwrap_or_default(),
                        src: attributes.get("src").cloned().unwrap_or_default(),
                    });
                } else if self.input[self.pos..].starts_with("<a") {
                    let (tag_end, attributes) = self.parse_tag_and_attributes(false);
                    self.pos = tag_end + 1;
                    self.update_position_for_tag_end(false);

                    let content_pos = Position {
                        line: self.line,
                        column: self.text_column,
                        offset: self.pos,
                    };

                    let text = self.parse_until("</a>");
                    self.consume_str("</a>", false);

                    result.push(Tag::Anchor {
                        href: attributes.get("href").cloned().unwrap_or_default(),
                        text,
                        position: content_pos,
                    });
                } else if self.input[self.pos..].starts_with("<span") {
                    let (tag_end, attributes) = self.parse_tag_and_attributes(false);
                    self.pos = tag_end + 1;
                    self.update_position_for_tag_end(false);
                    let text = self.parse_until("</span>");
                    self.consume_str("</span>", false);
                    result.push(Tag::Span { text, attributes });
                } else {
                    result.push(Tag::Text(self.consume_char(true).to_string()));
                }
            } else {
                let mut text = String::new();

                while self.pos < self.input.len() && !self.input[self.pos..].starts_with('<') {
                    let current_line = self.line;
                    let current_column = self.text_column;
                    let current_pos = self.pos;

                    if let Some((url, url_text)) = self.detect_markdown_link_at_current_position() {
                        if !text.is_empty() {
                            result.push(Tag::Text(text));
                            text = String::new();
                        }

                        let url_position = Position {
                            line: current_line,
                            column: current_column,
                            offset: current_pos,
                        };

                        result.push(Tag::Anchor {
                            href: url,
                            text: url_text,
                            position: url_position,
                        });
                    } else if let Some((url, url_text)) = self.detect_url_at_current_position() {
                        if !text.is_empty() {
                            result.push(Tag::Text(text));
                            text = String::new();
                        }

                        let url_position = Position {
                            line: current_line,
                            column: current_column,
                            offset: current_pos,
                        };

                        result.push(Tag::Anchor {
                            href: url,
                            text: url_text,
                            position: url_position,
                        });
                    } else {
                        text.push(self.consume_char(true));
                    }
                }

                if !text.is_empty() {
                    result.push(Tag::Text(text));
                }
            }
        }
        result
    }

    fn detect_markdown_link_at_current_position(&mut self) -> Option<(String, String)> {
        let remaining = &self.input[self.pos..];

        if !remaining.starts_with('[') {
            return None;
        }

        let mut bracket_depth = 0;
        let mut text_end = 0;

        for (i, c) in remaining.chars().enumerate().skip(1) {
            if c == '[' {
                bracket_depth += 1;
            } else if c == ']' {
                if bracket_depth == 0 {
                    text_end = i;
                    break;
                }
                bracket_depth -= 1;
            }
        }

        if text_end == 0 {
            return None;
        }

        if remaining.chars().nth(text_end + 1) != Some('(') {
            return None;
        }

        let link_text = remaining[1..text_end].to_string();

        let mut paren_depth = 0;
        let mut url_end = 0;

        for (i, c) in remaining.chars().enumerate().skip(text_end + 2) {
            if c == '(' {
                paren_depth += 1;
            } else if c == ')' {
                if paren_depth == 0 {
                    url_end = i;
                    break;
                }
                paren_depth -= 1;
            }
        }

        if url_end == 0 {
            return None;
        }

        let url = remaining[(text_end + 2)..url_end].to_string();

        for _ in 0..=url_end {
            self.consume_char(true);
        }

        Some((url, link_text))
    }

    fn detect_url_at_current_position(&mut self) -> Option<(String, String)> {
        let url_prefixes = ["http://", "https://"];

        let remaining = &self.input[self.pos..];

        for prefix in &url_prefixes {
            if remaining.starts_with(prefix) {
                let mut end_pos = 0;
                let mut depth = 0;

                for (i, c) in remaining.chars().enumerate() {
                    if i < prefix.len() {
                        continue;
                    }

                    match c {
                        ' ' | '\t' | '\n' | '\r' | '<' | '>' => {
                            break;
                        }
                        '(' | '[' | '{' => depth += 1,
                        ')' | ']' | '}' => {
                            if depth == 0 {
                                break;
                            }
                            depth -= 1;
                        }
                        '.' | ',' | ':' | ';' | '!' | '?' => {
                            if i + 1 >= remaining.len() {
                                end_pos = i;
                                break;
                            }
                        }
                        _ => {}
                    }

                    end_pos = i + 1;
                }

                if end_pos > 0 {
                    let url = remaining[..end_pos].to_string();

                    let url = url.trim_end_matches(|c| ".,:;!?".contains(c)).to_string();

                    (0..url.len()).for_each(|_| {
                        self.consume_char(true);
                    });

                    return Some((url.clone(), url));
                }
            }
        }

        None
    }

    fn parse_tag_and_attributes(
        &mut self,
        count_in_text: bool,
    ) -> (usize, HashMap<String, String>) {
        let mut attributes = HashMap::new();
        let mut in_attr_name = false;
        let mut in_attr_value = false;
        let mut attr_name = String::new();
        let mut attr_value = String::new();
        let mut quote_char = ' ';

        while self.pos < self.input.len()
            && !self.input[self.pos..].starts_with(' ')
            && self.input.chars().nth(self.pos) != Some('>')
        {
            self.consume_char(count_in_text);
        }

        while self.pos < self.input.len() && self.input.chars().nth(self.pos) != Some('>') {
            let current_char = self.consume_char(count_in_text);

            if in_attr_value {
                if current_char == quote_char {
                    in_attr_value = false;
                    attributes.insert(attr_name.clone(), attr_value.clone());
                    attr_name.clear();
                    attr_value.clear();
                } else {
                    attr_value.push(current_char);
                }
            } else if current_char.is_whitespace() {
                if in_attr_name && !attr_name.is_empty() {
                    in_attr_name = false;
                }
            } else if current_char == '=' {
                in_attr_name = false;
            } else if current_char == '"' || current_char == '\'' {
                in_attr_value = true;
                quote_char = current_char;
            } else {
                if !in_attr_name && attr_name.is_empty() {
                    in_attr_name = true;
                }

                if in_attr_name {
                    attr_name.push(current_char);
                }
            }
        }

        if self.pos < self.input.len() && self.input.chars().nth(self.pos) == Some('>') {
            self.consume_char(count_in_text);
        }

        (self.pos - 1, attributes)
    }

    fn parse_until(&mut self, end_marker: &str) -> String {
        let mut content = String::new();

        while self.pos < self.input.len() {
            let remaining = &self.input[self.pos..];
            if remaining.starts_with(end_marker) {
                return content;
            }

            content.push(self.consume_char(true));
        }

        content
    }

    fn consume_str(&mut self, s: &str, count_in_text: bool) {
        for c in s.chars() {
            if c == '\n' {
                self.line += 1;
                self.column = 0;
                if count_in_text {
                    self.text_column = 0;
                }
            } else {
                self.column += 1;
                if count_in_text {
                    self.text_column += 1;
                }
            }
            self.pos += c.len_utf8();
        }
    }

    fn consume_char(&mut self, count_in_text: bool) -> char {
        let mut iter = self.input[self.pos..].chars();
        let current_char = iter.next().unwrap();
        self.pos += current_char.len_utf8();

        if current_char == '\n' {
            self.line += 1;
            self.column = 0;
            if count_in_text {
                self.text_column = 0;
            }
        } else {
            self.column += 1;
            if count_in_text {
                self.text_column += 1;
            }
        }

        current_char
    }

    fn update_position_for_tag_end(&mut self, count_in_text: bool) {
        let slice = &self.input[..self.pos];
        let last_newline_pos = slice.rfind('\n');

        if let Some(pos) = last_newline_pos {
            self.line = slice[..pos].chars().filter(|&c| c == '\n').count() + 1;
            self.column = slice[pos + 1..].chars().count();

            if count_in_text {
                self.text_column = self.column;
            }
        } else {
            self.column = slice.chars().count();
            if count_in_text {
                self.text_column = self.column;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bold_tag() {
        let mut parser = Parser::new("<b>Bold text</b>".to_string());
        let result = parser.parse();
        assert_eq!(result.len(), 1);
        if let Tag::Bold(content) = &result[0] {
            assert_eq!(content, "Bold text");
        } else {
            panic!("Expected Bold tag");
        }
    }

    #[test]
    fn test_underline_tag() {
        let mut parser = Parser::new("<u>Underlined text</u>".to_string());
        let result = parser.parse();
        assert_eq!(result.len(), 1);
        if let Tag::Underline(content) = &result[0] {
            assert_eq!(content, "Underlined text");
        } else {
            panic!("Expected Underline tag");
        }
    }

    #[test]
    fn test_img_tag() {
        let mut parser = Parser::new("<img src=\"/tmp/image.png\" alt=\"Image\" />".to_string());
        let result = parser.parse();
        assert_eq!(result.len(), 1);
        if let Tag::Image { alt, src } = &result[0] {
            assert_eq!(alt, "Image");
            assert_eq!(src, "/tmp/image.png");
        } else {
            panic!("Expected Underline tag");
        }
    }

    #[test]
    fn test_text_with_newlines() {
        let mut parser = Parser::new("Line 1\nLine 2\n<b>Bold</b>".to_string());
        let result = parser.parse();
        assert_eq!(result.len(), 2);
        if let Tag::Text(content) = &result[0] {
            assert_eq!(content, "Line 1\nLine 2\n");
        } else {
            panic!("Expected Text tag");
        }
        if let Tag::Bold(content) = &result[1] {
            assert_eq!(content, "Bold");
        } else {
            panic!("Expected Bold tag");
        }
    }

    #[test]
    fn test_mixed_content() {
        let html = "Text <b>bold</b> more <i>italic</i>";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();
        assert_eq!(result.len(), 4);
        if let Tag::Text(content) = &result[0] {
            assert_eq!(content, "Text ");
        } else {
            panic!("Expected Text tag");
        }
        if let Tag::Bold(content) = &result[1] {
            assert_eq!(content, "bold");
        } else {
            panic!("Expected Bold tag");
        }
        if let Tag::Text(content) = &result[2] {
            assert_eq!(content, " more ");
        } else {
            panic!("Expected Text tag");
        }
        if let Tag::Italic(content) = &result[3] {
            assert_eq!(content, "italic");
        } else {
            panic!("Expected Italic tag");
        }
    }

    #[test]
    fn test_anchor_tag_with_position() {
        let html = "<a href=\"https://example.com\">Link text</a>";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();
        assert_eq!(result.len(), 1);
        if let Tag::Anchor {
            href,
            text,
            position,
        } = &result[0]
        {
            assert_eq!(href, "https://example.com");
            assert_eq!(text, "Link text");
            assert_eq!(position.line, 0);
            assert_eq!(position.column, 0);
        } else {
            panic!("Expected Anchor tag");
        }
    }

    #[test]
    fn test_multiline_with_anchor() {
        let html = "Line 1\n<a href=\"link\">Line 2</a>\nLine 3";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();
        assert_eq!(result.len(), 3);
        if let Tag::Text(content) = &result[0] {
            assert_eq!(content, "Line 1\n");
        } else {
            panic!("Expected Text tag");
        }
        if let Tag::Anchor { text, position, .. } = &result[1] {
            assert_eq!(text, "Line 2");
            assert_eq!(position.line, 1);
            assert_eq!(position.column, 0);
        } else {
            panic!("Expected Anchor tag");
        }
        if let Tag::Text(content) = &result[2] {
            assert_eq!(content, "\nLine 3");
        } else {
            panic!("Expected Text tag");
        }
    }

    #[test]
    fn test_text_column_counting() {
        let html = "Text <b>bold</b> <a href=\"link\">anchor</a> after";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        assert_eq!(result.len(), 5);

        if let Tag::Anchor { position, .. } = &result[3] {
            assert_eq!(position.column, 10);
        } else {
            panic!("Expected Anchor tag");
        }
    }

    #[test]
    fn test_complex_markup_with_positions() {
        let html =
            "<b>Bold</b> text <i>and</i> <a href=\"#1\">link 1</a> and <a href=\"#2\">link 2</a>";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        assert_eq!(result.len(), 7);

        if let Tag::Anchor { text, position, .. } = &result[4] {
            assert_eq!(text, "link 1");
            assert_eq!(position.column, 14);
        } else {
            panic!("Expected first Anchor tag");
        }

        if let Tag::Anchor { text, position, .. } = &result[6] {
            assert_eq!(text, "link 2");
            assert_eq!(position.column, 25);
        } else {
            panic!("Expected second Anchor tag");
        }
    }

    #[test]
    fn test_complex_multiline_markup_with_positions() {
        let html = "<u>underline</u>\n<i>italic</i>\n<b>bold</b>\n<a href=\"https://github.com/unixpariah/moxnotify\">github</a>\n<img alt=\"image\" href=\"\"/>";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        assert_eq!(result.len(), 9, "Expected 9 tags in total");

        if let Tag::Underline(content) = &result[0] {
            assert_eq!(content, "underline", "Expected underline content");
        } else {
            panic!("Expected Underline tag at position 0");
        }

        if let Tag::Text(content) = &result[1] {
            assert_eq!(content, "\n", "Expected newline after underline");
        } else {
            panic!("Expected Text tag with newline at position 1");
        }

        if let Tag::Italic(content) = &result[2] {
            assert_eq!(content, "italic", "Expected italic content");
        } else {
            panic!("Expected Italic tag at position 2");
        }

        if let Tag::Text(content) = &result[3] {
            assert_eq!(content, "\n", "Expected newline after italic");
        } else {
            panic!("Expected Text tag with newline at position 3");
        }

        if let Tag::Bold(content) = &result[4] {
            assert_eq!(content, "bold", "Expected bold content");
        } else {
            panic!("Expected Bold tag at position 4");
        }

        if let Tag::Text(content) = &result[5] {
            assert_eq!(content, "\n", "Expected newline after bold");
        } else {
            panic!("Expected Text tag with newline at position 5");
        }

        if let Tag::Anchor {
            href,
            text,
            position,
        } = &result[6]
        {
            assert_eq!(
                href, "https://github.com/unixpariah/moxnotify",
                "Expected correct href"
            );
            assert_eq!(text, "github", "Expected anchor text");
            assert_eq!(position.line, 3, "Expected anchor on line 3");
            assert_eq!(position.column, 0)
        } else {
            panic!("Expected Anchor tag at position 6");
        }

        if let Tag::Text(content) = &result[7] {
            assert_eq!(content, "\n", "Expected newline after anchor");
        } else {
            panic!("Expected Text tag with newline at position 7");
        }

        if let Tag::Image { alt, src } = &result[8] {
            assert_eq!(alt, "image", "Expected image alt text");
            assert_eq!(src, "", "Expected empty src attribute");
        } else {
            panic!("Expected Image tag at position 8");
        }
    }

    #[test]
    fn test_plain_url_detection() {
        let html = "Check out https://example.com for more info.";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        assert_eq!(result.len(), 3);

        if let Tag::Text(content) = &result[0] {
            assert_eq!(content, "Check out ");
        } else {
            panic!("Expected Text tag at position 0");
        }

        if let Tag::Anchor {
            href,
            text,
            position,
        } = &result[1]
        {
            assert_eq!(href, "https://example.com");
            assert_eq!(text, "https://example.com");
            assert_eq!(position.line, 0);
            assert_eq!(position.column, 10);
        } else {
            panic!("Expected Anchor tag at position 1");
        }

        if let Tag::Text(content) = &result[2] {
            assert_eq!(content, " for more info.");
        } else {
            panic!("Expected Text tag at position 2");
        }
    }

    #[test]
    fn test_multiple_urls() {
        let html = "First: https://example.com and second: http://test.org!";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        assert_eq!(result.len(), 5);

        if let Tag::Anchor { href, .. } = &result[1] {
            assert_eq!(href, "https://example.com");
        } else {
            panic!("Expected first Anchor tag");
        }

        if let Tag::Anchor { href, .. } = &result[3] {
            assert_eq!(href, "http://test.org");
        } else {
            panic!("Expected second Anchor tag");
        }
    }

    #[test]
    fn test_url_with_path() {
        let html = "Check https://example.com/path/to/page?query=value";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        if let Tag::Anchor { href, .. } = &result[1] {
            assert_eq!(href, "https://example.com/path/to/page?query=value");
        } else {
            panic!("Expected Anchor tag");
        }
    }

    #[test]
    fn test_url_and_html_tags_mixed() {
        let html = "<b>Bold</b> and https://example.com and <i>italic</i>";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        assert_eq!(result.len(), 5);

        if let Tag::Bold(content) = &result[0] {
            assert_eq!(content, "Bold");
        } else {
            panic!("Expected Bold tag");
        }

        if let Tag::Anchor { href, .. } = &result[2] {
            assert_eq!(href, "https://example.com");
        } else {
            panic!("Expected Anchor tag");
        }

        if let Tag::Italic(content) = &result[4] {
            assert_eq!(content, "italic");
        } else {
            panic!("Expected Italic tag");
        }
    }

    #[test]
    fn test_span() {
        let html = "text <span>Span</span>";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        if let Tag::Text(content) = &result[0] {
            assert_eq!(content, "text ");
        } else {
            panic!("Expected text tag");
        }

        if let Tag::Span {
            text,
            attributes: _,
        } = &result[1]
        {
            assert_eq!(text, "Span");
        } else {
            panic!("Expected span tag");
        }
    }

    #[test]
    fn test_markdown_link() {
        let html = "Check out [Example Site](https://example.com) for more info.";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        assert_eq!(result.len(), 3);

        if let Tag::Text(content) = &result[0] {
            assert_eq!(content, "Check out ");
        } else {
            panic!("Expected Text tag at position 0");
        }

        if let Tag::Anchor {
            href,
            text,
            position,
        } = &result[1]
        {
            assert_eq!(href, "https://example.com");
            assert_eq!(text, "Example Site");
            assert_eq!(position.line, 0);
            assert_eq!(position.column, 10);
        } else {
            panic!("Expected Anchor tag at position 1");
        }

        if let Tag::Text(content) = &result[2] {
            assert_eq!(content, " for more info.");
        } else {
            panic!("Expected Text tag at position 2");
        }
    }

    #[test]
    fn test_multiple_markdown_links() {
        let html = "First: [Example](https://example.com) and second: [Test](http://test.org)!";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        assert_eq!(result.len(), 5);

        if let Tag::Anchor { href, text, .. } = &result[1] {
            assert_eq!(href, "https://example.com");
            assert_eq!(text, "Example");
        } else {
            panic!("Expected first Anchor tag");
        }

        if let Tag::Anchor { href, text, .. } = &result[3] {
            assert_eq!(href, "http://test.org");
            assert_eq!(text, "Test");
        } else {
            panic!("Expected second Anchor tag");
        }
    }

    #[test]
    fn test_mixed_link_types() {
        let html = "<b>Bold</b> and [Example](https://example.com) and <i>italic</i> and https://plainurl.com";
        let mut parser = Parser::new(html.to_string());
        let result = parser.parse();

        if let Tag::Bold(content) = &result[0] {
            assert_eq!(content, "Bold");
        } else {
            panic!("Expected Bold tag");
        }

        if let Tag::Anchor { href, text, .. } = &result[2] {
            assert_eq!(href, "https://example.com");
            assert_eq!(text, "Example");
        } else {
            panic!("Expected Markdown Anchor tag");
        }

        if let Tag::Italic(content) = &result[4] {
            assert_eq!(content, "italic");
        } else {
            panic!("Expected Italic tag");
        }

        if let Tag::Anchor { href, text, .. } = &result[6] {
            assert_eq!(href, "https://plainurl.com");
            assert_eq!(text, "https://plainurl.com");
        } else {
            panic!("Expected Plain URL Anchor tag");
        }
    }
}
