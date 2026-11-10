use log4rs::encode::{Style, Write};
use regex::Regex;

pub(super) fn compile(expressions: &[String]) -> anyhow::Result<Option<Regex>> {
    if expressions.is_empty() { return Ok(None); }
    let pattern = expressions.iter().map(|expression| format!("(?:{expression})"))
        .collect::<Vec<_>>().join("|");
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
        let mut matches = regex.find_iter(text).peekable();
        let mut cursor = 0;
        for (end, control) in self.controls.iter().map(|(offset, control)| (*offset, Some(control)))
            .chain(std::iter::once((text.len(), None))) {
            while cursor < end {
                while matches.peek().is_some_and(|found| found.end() <= cursor) { matches.next(); }
                if let Some(found) = matches.peek() {
                    let keep_end = found.start().min(end).max(cursor);
                    writer.write_all(&self.text[cursor..keep_end])?;
                    cursor = if found.start() < end { found.end().min(end).max(keep_end) } else { end };
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

    #[test]
    fn combined_patterns_handle_alternation_flags_empty_matches_and_unicode() {
        for (patterns, text, expected) in [
            (vec!["ab|cd", "é+"], "abécdé kept", " kept"),
            (vec!["(?i)drop", "KEEP"], "DROP drop KEEP keep", "   keep"),
            (vec!["", "x"], "xyz", "xyz"),
            (vec!["^", "$"], "é", "é"),
            (vec![".+"], "all gone", ""),
        ] {
            let regex = compile(&patterns.into_iter().map(str::to_owned).collect::<Vec<_>>()).unwrap().unwrap();
            let mut line = RenderedLine::default();
            std::io::Write::write_all(&mut line, text.as_bytes()).unwrap();
            let mut output = Vec::new();
            line.emit(&mut SimpleWriter(&mut output), &regex).unwrap();
            assert_eq!(output, expected.as_bytes());
        }
        assert!(compile(&[]).unwrap().is_none());
        assert!(compile(&["[".to_owned()]).is_err());
    }
}
