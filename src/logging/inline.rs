use std::{borrow::Cow, fmt::{self, Write}};

pub(crate) const COLORS_KEY: &str = "inline_colors";

#[doc(hidden)]
pub enum Segment<'a> {
    Text(fmt::Arguments<'a>),
    Palette(&'a str),
}

// Ordered changes use UTF-8 byte offsets into the plain message. A null palette
// restores normal text; names resolve against each consumer's configured palette.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ColorChange<'a> {
    #[serde(rename = "utf8_offset")]
    pub offset: usize,
    #[serde(borrow)]
    pub palette: Option<Cow<'a, str>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_attributes_preserve_utf8_offsets_escaped_names_and_same_position_changes() {
        let name = "a\"b\\c\n";
        let message = Message::new(&[
            Segment::Palette("warn"), Segment::Text(format_args!("界é")),
            Segment::Palette(name), Segment::Palette(""), Segment::Text(format_args!("!")),
        ]);
        assert_eq!(message.text, "界é!");
        assert_eq!(serde_json::from_str::<serde_json::Value>(&message.colors).unwrap(), serde_json::json!([
            {"utf8_offset": 0, "palette": "warn"},
            {"utf8_offset": 5, "palette": name},
            {"utf8_offset": 5, "palette": null}
        ]));
        let changes: Vec<ColorChange<'_>> = serde_json::from_str(&message.colors).unwrap();
        assert_eq!(changes[1].palette.as_deref(), Some(name));
    }
}

#[doc(hidden)]
pub struct Message {
    pub text: String,
    pub colors: String,
}

impl Message {
    pub fn new(segments: &[Segment<'_>]) -> Self {
        let mut text = String::new();
        let mut changes = Vec::new();
        for segment in segments {
            match segment {
                Segment::Text(args) => write!(text, "{args}").expect("inline message formatting failed"),
                Segment::Palette(name) => changes.push(ColorChange {
                    offset: text.len(), palette: if name.is_empty() { None } else { Some((*name).into()) },
                }),
            }
        }
        Self { text, colors: serde_json::to_string(&changes).expect("inline colors serialization failed") }
    }
}
