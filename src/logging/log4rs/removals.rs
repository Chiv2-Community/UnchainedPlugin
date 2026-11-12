use itertools::Itertools;
use log4rs::encode::{Style, Write};
use regex::Regex;

pub(super) fn compile(expressions: &[String]) -> anyhow::Result<Option<Regex>> {
    if expressions.is_empty() { return Ok(None); }
    let pattern = expressions.iter().map(|expression| format!("(?:{expression})")).join("|");
    Ok(Some(Regex::new(&pattern)?))
}

enum Control {
    Style(Style),
    Ansi(Vec<u8>),
}

// Keep controls outside searchable text, including matches spanning color changes.
#[derive(Default)]
pub(super) struct RenderedLine {
    text: Vec<u8>,
    controls: Vec<(usize, Control)>,
}

impl RenderedLine {
    pub(super) fn ansi(&mut self, bytes: &[u8]) {
        self.controls.push((self.text.len(), Control::Ansi(bytes.to_vec())));
    }

    pub(super) fn emit(&self, writer: &mut dyn Write, regex: &Regex) -> anyhow::Result<()> {
        let text = std::str::from_utf8(&self.text)?;
        self.emit_ranges(writer, regex.find_iter(text).map(|found| found.range()))
    }

    pub(super) fn emit_without_emojis(&self, writer: &mut dyn Write) -> anyhow::Result<()> {
        use unicode_segmentation::UnicodeSegmentation;
        let text = std::str::from_utf8(&self.text)?;
        let emojis = text.grapheme_indices(true)
            .filter(|(_, grapheme)| emojis::get(grapheme).is_some())
            .map(|(start, grapheme)| start..start + grapheme.len());
        self.emit_ranges(writer, emojis)
    }

    fn emit_ranges(&self, writer: &mut dyn Write,
        ranges: impl Iterator<Item = std::ops::Range<usize>>) -> anyhow::Result<()> {
        let mut matches = ranges.peekable();
        let mut cursor = 0;
        for (end, control) in self.controls.iter().map(|(offset, control)| (*offset, Some(control)))
            .chain(std::iter::once((self.text.len(), None))) {
            while cursor < end {
                while matches.peek().is_some_and(|found| found.end <= cursor) { matches.next(); }
                if let Some(found) = matches.peek() {
                    let keep_end = found.start.clamp(cursor, end);
                    writer.write_all(&self.text[cursor..keep_end])?;
                    cursor = found.end.clamp(keep_end, end);
                } else {
                    writer.write_all(&self.text[cursor..end])?;
                    cursor = end;
                }
            }
            match control {
                Some(Control::Style(style)) => writer.set_style(style)?,
                Some(Control::Ansi(bytes)) => writer.write_all(bytes)?,
                None => {},
            }
        }
        Ok(())
    }
}

impl std::io::Write for RenderedLine {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.text.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

impl Write for RenderedLine {
    fn set_style(&mut self, style: &Style) -> std::io::Result<()> {
        self.controls.push((self.text.len(), Control::Style(style.clone())));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log4rs::encode::writer::simple::SimpleWriter;

    #[test_case::test_case(&["ab|cd", "é+"], "abécdé kept", " kept"; "alternation_and_unicode")]
    #[test_case::test_case(&["(?i)drop", "KEEP"], "DROP drop KEEP keep", "   keep"; "scoped_flags")]
    #[test_case::test_case(&["", "x"], "xyz", "xyz"; "empty_alternative")]
    #[test_case::test_case(&[".+"], "all gone", ""; "entire_line")]
    fn removal_patterns_preserve_regex_semantics(patterns: &[&str], text: &str, expected: &str) {
        let regex = compile(&patterns.iter().map(|pattern| (*pattern).to_owned()).collect::<Vec<_>>()).unwrap().unwrap();
        let mut line = RenderedLine::default();
        std::io::Write::write_all(&mut line, text.as_bytes()).unwrap();
        let mut output = Vec::new();
        line.emit(&mut SimpleWriter(&mut output), &regex).unwrap();
        assert_eq!(output, expected.as_bytes());
    }

    #[test]
    fn empty_configuration_disables_removals() {
        assert!(compile(&[]).unwrap().is_none());
    }

    #[test_case::test_case("DROP[0-9]+"; "single_match")]
    #[test_case::test_case("DROP[0-9]+|"; "empty_matches")]
    #[test_case::test_case("D|RO|P|[0-9]+"; "adjacent_matches")]
    fn removals_span_multiple_style_boundaries(pattern: &str) {
        use std::io::Write as _;
        let mut line = RenderedLine::default();
        for text in ["éD", "RO", "P42尾"] {
            line.write_all(text.as_bytes()).unwrap();
            line.set_style(&Style::new()).unwrap();
        }
        let mut output = RenderedLine::default();
        line.emit(&mut output, &Regex::new(pattern).unwrap()).unwrap();
        assert_eq!(output.text, "é尾".as_bytes());
        assert_eq!(output.controls.iter().map(|(offset, _)| *offset).collect::<Vec<_>>(), [2, 2, 5]);
    }

    #[test]
    fn emoji_removal_preserves_styles_ansi_and_nonemoji_unicode() {
        use std::io::Write as _;
        let mut line = RenderedLine::default();
        line.write_all("A👩".as_bytes()).unwrap();
        let mut style = Style::new();
        style.text(log4rs::encode::Color::Red);
        line.set_style(&style).unwrap();
        line.write_all("\u{200d}💻雪🇩🇪e\u{301}".as_bytes()).unwrap();
        line.ansi(b"\x1b[0m");
        line.write_all(b"\t\r\n").unwrap();
        let mut output = RenderedLine::default();
        line.emit_without_emojis(&mut output).unwrap();
        assert_eq!(output.text, "A雪e\u{301}\x1b[0m\t\r\n".as_bytes());
        assert_eq!(output.controls.len(), 1);
        assert_eq!(output.controls[0].0, 1);
        assert!(matches!(&output.controls[0].1, Control::Style(actual) if *actual == style));
    }
}
