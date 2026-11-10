//! Structured text with backend-specific formatting. Text leaves are literal, not markup.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichText(pub Vec<RichTextChunk>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RichTextChunk {
    Text(String),
    Bold(RichText),
    Italics(RichText),
    Underline(RichText),
    /// RGB color; currently ignored by all renderers.
    Color(u32, RichText),
}

#[derive(Clone, Copy)]
enum Format {
    Plain,
    Discord,
    Game,
}

impl RichText {
    /// Append a chunk, another rich text value, or literal text.
    pub fn append(&mut self, text: impl Into<RichText>) -> &mut Self {
        self.0.extend(text.into().0);
        self
    }

    /// Parse inline Discord-style Markdown: **bold**, *italic*, _italic_, __underline__.
    /// Supports nesting and escaped punctuation, not full Markdown blocks or links.
    pub fn from_markdown(text: impl AsRef<str>) -> Self {
        MarkdownParser {
            text: text.as_ref(),
            position: 0,
        }
        .parse(None, 0)
        .0
    }

    /// Parse a Markdown template, then insert literal values at `{}` placeholders.
    /// Format values before passing them if precision or other formatting is needed.
    pub fn from_markdown_template(template: &str, values: &[String]) -> Self {
        fn substitute(text: &mut RichText, values: &mut std::slice::Iter<'_, String>) {
            for chunk in &mut text.0 {
                match chunk {
                    RichTextChunk::Text(text) => {
                        let mut parts = text.split("{}");
                        let mut result = parts.next().unwrap_or_default().to_owned();
                        for part in parts {
                            result.push_str(values.next().expect("missing rich_text! argument"));
                            result.push_str(part);
                        }
                        *text = result;
                    }
                    RichTextChunk::Bold(text)
                    | RichTextChunk::Italics(text)
                    | RichTextChunk::Underline(text)
                    | RichTextChunk::Color(_, text) => substitute(text, values),
                }
            }
        }
        let mut text = Self::from_markdown(template);
        let mut values = values.iter();
        substitute(&mut text, &mut values);
        assert!(values.next().is_none(), "unused rich_text! argument");
        text
    }

    pub fn to_plain_text(&self) -> String {
        self.render(Format::Plain)
    }

    pub fn to_discord(&self) -> String {
        self.render(Format::Discord)
    }

    pub fn to_game_chat(&self) -> String {
        self.render(Format::Game)
    }

    fn render(&self, format: Format) -> String {
        let mut output = String::new();
        self.write(&mut output, format, 0);
        output
    }

    fn write(&self, output: &mut String, format: Format, active: u8) {
        for chunk in &self.0 {
            let (children, marker, flag) = match chunk {
                RichTextChunk::Text(text) => {
                    for ch in text.chars() {
                        match format {
                            Format::Game => match ch {
                                '&' => output.push_str("&amp;"),
                                '<' => output.push_str("&lt;"),
                                '>' => output.push_str("&gt;"),
                                '"' => output.push_str("&quot;"),
                                _ => output.push(ch),
                            },
                            Format::Discord => {
                                if matches!(
                                    ch,
                                    '\\' | '*' | '_' | '~' | '`' | '|' | '>' | '#' | '[' | ']'
                                ) {
                                    output.push('\\');
                                }
                                output.push(ch);
                            }
                            Format::Plain => output.push(ch),
                        }
                    }
                    continue;
                }
                RichTextChunk::Bold(children) => (children, "**", 1),
                RichTextChunk::Italics(children) => (children, "*", 2),
                RichTextChunk::Underline(children) => (children, "__", 4),
                RichTextChunk::Color(_, children) => (children, "", 0),
            };
            match format {
                Format::Discord => {
                    let marker = if active & flag != 0 { "" } else { marker };
                    let start = output.len();
                    output.push_str(marker);
                    let content_start = output.len();
                    children.write(output, format, active | flag);
                    if output.len() == content_start {
                        output.truncate(start);
                    } else {
                        output.push_str(marker);
                    }
                }
                Format::Game if flag == 1 && active & 1 == 0 => {
                    // The default Slate parser does not span newlines or compose nested tags.
                    let mut text = String::new();
                    children.write(&mut text, format, active | flag);
                    for (index, line) in text.split('\n').enumerate() {
                        if index > 0 {
                            output.push('\n');
                        }
                        if !line.is_empty() {
                            output.push_str("<RichText.Emphasis>");
                            output.push_str(line);
                            output.push_str("</>");
                        }
                    }
                }
                _ => children.write(output, format, active),
            }
        }
    }
}

/// Parse inline Markdown into structured text. String conversions remain literal.
///
/// ```
/// let text = UnchainedPlugin::rich_text!("**Bold** and *italic*");
/// assert_eq!(text.to_plain_text(), "Bold and italic");
/// let name = "Player_one";
/// let text = UnchainedPlugin::rich_text!("**{}** has {} kills", name, 5);
/// assert_eq!(text.to_plain_text(), "Player_one has 5 kills");
/// ```
/// Arguments fill sequential `{}` placeholders as literal text, not Markdown.
/// Named placeholders and format specifiers are unsupported; preformat values instead.
#[macro_export]
macro_rules! rich_text {
    ($template:literal, $($value:expr),+ $(,)?) => {
        $crate::events::rich_text::RichText::from_markdown_template(
            $template, &[$(::std::format!("{}", $value)),+],
        )
    };
    ($text:expr $(,)?) => {
        $crate::events::rich_text::RichText::from_markdown($text)
    };
}

struct MarkdownParser<'a> {
    text: &'a str,
    position: usize,
}

impl MarkdownParser<'_> {
    fn parse(&mut self, closing: Option<&str>, depth: usize) -> (RichText, bool) {
        let mut chunks = Vec::new();
        let mut literal = String::new();
        while self.position < self.text.len() {
            let rest = &self.text[self.position..];
            if let Some(marker) = closing.filter(|marker| rest.starts_with(marker)) {
                self.position += marker.len();
                Self::flush(&mut chunks, &mut literal);
                return (RichText(chunks), true);
            }
            let ch = rest.chars().next().unwrap();
            if ch == '\\' {
                if let Some(escaped) = rest[1..].chars().next().filter(char::is_ascii_punctuation) {
                    literal.push(escaped);
                    self.position += 1 + escaped.len_utf8();
                    continue;
                }
            }
            let marker = ["***", "**", "__", "*", "_"]
                .into_iter()
                .find(|marker| rest.starts_with(marker));
            // Bound recursion for untrusted input; deeper markers remain literal.
            if let Some(marker) = marker.filter(|_| depth < 64) {
                self.position += marker.len();
                Self::flush(&mut chunks, &mut literal);
                let (children, matched) = self.parse(Some(marker), depth + 1);
                if matched {
                    chunks.push(match marker {
                        "***" => RichTextChunk::Bold(RichTextChunk::Italics(children).into()),
                        "**" => RichTextChunk::Bold(children),
                        "__" => RichTextChunk::Underline(children),
                        _ => RichTextChunk::Italics(children),
                    });
                } else {
                    chunks.push(RichTextChunk::Text(marker.into()));
                    chunks.extend(children.0);
                }
            } else {
                literal.push(ch);
                self.position += ch.len_utf8();
            }
        }
        Self::flush(&mut chunks, &mut literal);
        (RichText(chunks), false)
    }

    fn flush(chunks: &mut Vec<RichTextChunk>, literal: &mut String) {
        if !literal.is_empty() {
            chunks.push(RichTextChunk::Text(std::mem::take(literal)));
        }
    }
}

impl From<String> for RichText {
    fn from(text: String) -> Self {
        Self(vec![RichTextChunk::Text(text)])
    }
}

impl From<&str> for RichText {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}

impl From<Vec<RichTextChunk>> for RichText {
    fn from(chunks: Vec<RichTextChunk>) -> Self {
        Self(chunks)
    }
}

impl From<RichTextChunk> for RichText {
    fn from(chunk: RichTextChunk) -> Self {
        Self(vec![chunk])
    }
}

impl<T: Into<RichText>> std::ops::Add<T> for RichText {
    type Output = Self;

    fn add(mut self, text: T) -> Self {
        self.append(text);
        self
    }
}

impl<T: Into<RichText>> std::ops::AddAssign<T> for RichText {
    fn add_assign(&mut self, text: T) {
        self.append(text);
    }
}

#[cfg(test)]
mod tests {
    use super::RichTextChunk::*;
    use super::*;

    #[test]
    fn append_and_add_accept_chunks_and_rich_text() {
        let mut text = RichText::default();
        text.append(Text("Hello ".into()))
            .append(rich_text!("**world**"));
        text += Text("! ".into());
        text += rich_text!("*Again*");
        let text = text + Text(" and ".into()) + rich_text!("__again__");
        assert_eq!(text.to_plain_text(), "Hello world! Again and again");
        assert_eq!(text.to_discord(), "Hello **world**! *Again* and __again__");
        assert_eq!(
            text.to_game_chat(),
            "Hello <RichText.Emphasis>world</>! Again and again"
        );
    }

    #[test]
    fn appended_strings_are_literal() {
        let mut text = rich_text!("**start**") + " *literal*";
        text += String::from(" __literal__");
        text.append(" **literal**");
        assert_eq!(
            text.to_plain_text(),
            "start *literal* __literal__ **literal**"
        );
    }

    #[test]
    fn markdown_macro_models_nested_chunks() {
        assert_eq!(
            rich_text!("Hello **bold *italic* and __underlined__**!"),
            RichText(vec![
                Text("Hello ".into()),
                Bold(RichText(vec![
                    Text("bold ".into()),
                    Italics("italic".into()),
                    Text(" and ".into()),
                    Underline("underlined".into()),
                ])),
                Text("!".into()),
            ]),
        );
        let text = rich_text!("***both*** and _italic_");
        assert_eq!(text.to_discord(), "***both*** and *italic*");
        assert_eq!(text.to_game_chat(), "<RichText.Emphasis>both</> and italic");
    }

    #[test]
    fn markdown_escapes_unmatched_markers_and_unicode() {
        for input in [
            "",
            "plain 🦀",
            "**unclosed",
            "trailing\\",
            "a **b *c",
            "__x",
        ] {
            assert_eq!(rich_text!(input).to_plain_text(), input);
        }
        assert_eq!(
            rich_text!(r"\*literal\* and **雪**").to_plain_text(),
            "*literal* and 雪"
        );
        assert_eq!(
            rich_text!(r"\*literal\* and **雪**").to_discord(),
            r"\*literal\* and **雪**"
        );
        assert_eq!(
            rich_text!(String::from("**owned**"),).to_plain_text(),
            "owned"
        );
    }

    #[test]
    fn macro_interpolation_keeps_values_literal() {
        let name = "**雪**_player_{}\\";
        let text = rich_text!("**{}** has {} kills", name, 3);
        assert_eq!(text.to_plain_text(), format!("{name} has 3 kills"));
        assert_eq!(text.0[0], Bold(name.into()));
        let empty = rich_text!("*{}* **{}** __{}__", "", "", "");
        assert_eq!(empty.to_plain_text(), "  ");
        assert_eq!(empty.to_discord(), "  ");
        assert_eq!(empty.to_game_chat(), "  ");
        assert_eq!(
            rich_text!("{}", "**not bold**"),
            RichText::from("**not bold**")
        );
    }

    #[test]
    fn backend_formats_and_color_noop() {
        let text = RichText(vec![
            Text("Hello ".into()),
            Bold(Italics(Underline(Color(0xff0000, "world".into()).into()).into()).into()),
        ]);
        assert_eq!(text.to_plain_text(), "Hello world");
        assert_eq!(text.to_discord(), "Hello ***__world__***");
        assert_eq!(text.to_game_chat(), "Hello <RichText.Emphasis>world</>");
    }

    #[test]
    fn text_is_literal() {
        let text = RichText::from("<tag>**_x_** & \"y\"");
        assert_eq!(text.to_plain_text(), "<tag>**_x_** & \"y\"");
        assert_eq!(text.to_discord(), "<tag\\>\\*\\*\\_x\\_\\*\\* & \"y\"");
        assert_eq!(
            text.to_game_chat(),
            "&lt;tag&gt;**_x_** &amp; &quot;y&quot;"
        );
    }

    #[test]
    fn game_bold_is_flat_and_line_scoped() {
        let text = RichText::from(Bold(RichText(vec![
            Text("a\n".into()),
            Bold("b".into()),
            Text("\n".into()),
        ])));
        assert_eq!(
            text.to_game_chat(),
            "<RichText.Emphasis>a</>\n<RichText.Emphasis>b</>\n"
        );
        assert_eq!(RichText::default().to_game_chat(), "");
        assert_eq!(
            RichText::from(Color(0, "plain".into())).to_discord(),
            "plain"
        );
        assert_eq!(
            RichText::from(Bold(Bold("x".into()).into())).to_discord(),
            "**x**"
        );
    }
}
