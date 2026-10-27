use std::io;

use log4rs::encode::{writer::{console::ConsoleWriter, simple::SimpleWriter}, Color, Style, Write};

const TITLE: &str = "\
\
▄████████    ▄█    █▄     ▄█   ▄█    █▄     ▄████████  ▄█          ▄████████ ▄██   ▄         ▄█   ▄█ \r\n\
███    ███   ███    ███   ███  ███    ███   ███    ███ ███         ███    ███ ███   ██▄      ███  ███ \r\n\
███    █▀    ███    ███   ███▌ ███    ███   ███    ███ ███         ███    ███ ███▄▄▄███      ███▌ ███▌\r\n\
███         ▄███▄▄▄▄███▄▄ ███▌ ███    ███   ███    ███ ███        ▄███▄▄▄▄██▀ ▀▀▀▀▀▀███      ███▌ ███▌\r\n\
███        ▀▀███▀▀▀▀███▀  ███▌ ███    ███ ▀███████████ ███       ▀▀███▀▀▀▀▀   ▄██   ███      ███▌ ███▌\r\n\
███    █▄    ███    ███   ███  ███    ███   ███    ███ ███       ▀███████████ ███   ███      ███  ███ \r\n\
███    ███   ███    ███   ███  ███    ███   ███    ███ ███▌    ▄   ███    ███ ███   ███      ███  ███ \r\n\
████████▀    ███    █▀    █▀    ▀██████▀    ███    █▀  █████▄▄██   ███    ███  ▀█████▀       █▀   █▀  \r\n\
                                                       ▀           ███    ███                         \r\n\
";

const SUBTITLE: &str = "███    █▄  ███▄▄▄▄    ▄████████    ▄█    █▄       ▄████████  ▄█  ███▄▄▄▄      ▄████████ ████████▄     \r\n\
███    ███ ███▀▀▀██▄ ███    ███   ███    ███     ███    ███ ███  ███▀▀▀██▄   ███    ███ ███   ▀███    \r\n\
███    ███ ███   ███ ███    █▀    ███    ███     ███    ███ ███▌ ███   ███   ███    █▀  ███    ███    \r\n\
███    ███ ███   ███ ███         ▄███▄▄▄▄███▄▄   ███    ███ ███▌ ███   ███  ▄███▄▄▄     ███    ███    \r\n\
███    ███ ███   ███ ███        ▀▀███▀▀▀▀███▀  ▀███████████ ███▌ ███   ███ ▀▀███▀▀▀     ███    ███    \r\n\
███    ███ ███   ███ ███    █▄    ███    ███     ███    ███ ███  ███   ███   ███    █▄  ███    ███    \r\n\
███    ███ ███   ███ ███    ███   ███    ███     ███    ███ ███  ███   ███   ███    ███ ███   ▄███    \r\n\
████████▀   ▀█   █▀  ████████▀    ███    █▀      ███    █▀  █▀    ▀█   █▀    ██████████ ████████▀     \r\n\
";

pub(crate) fn print_banner() -> io::Result<()> {
    if let Some(console) = ConsoleWriter::stdout() {
        write_banner(&mut console.lock())
    } else {
        write_banner(&mut SimpleWriter(io::stdout().lock()))
    }
}

fn write_banner(writer: &mut dyn Write) -> io::Result<()> {
    let result: io::Result<()> = (|| {
        for (text, color) in [(TITLE, Color::Yellow), (SUBTITLE, Color::Red)] {
            let mut style = Style::new();
            style.text(color).intense(true);
            writer.set_style(&style)?;
            writer.write_all(text.as_bytes())?;
        }
        Ok(())
    })();
    let reset = writer.set_style(&Style::new());
    result?;
    reset?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_banner_has_no_ansi_or_unterminated_lines() {
        let mut bytes = Vec::new();
        write_banner(&mut SimpleWriter(&mut bytes)).unwrap();
        assert_eq!(bytes, format!("{TITLE}{SUBTITLE}").as_bytes());
        assert!(!bytes.contains(&0x1b));
        assert!(bytes.ends_with(b"\r\n"));
        assert_eq!(bytes.iter().filter(|&&byte| byte == b'\n').count(), 17);
    }

    #[derive(Default)]
    struct StyledBuffer {
        bytes: Vec<u8>,
        styles: Vec<(usize, Style)>,
        fail: bool,
    }

    impl io::Write for StyledBuffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail { return Err(io::Error::other("injected failure")); }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    impl Write for StyledBuffer {
        fn set_style(&mut self, style: &Style) -> io::Result<()> {
            self.styles.push((self.bytes.len(), style.clone()));
            Ok(())
        }
    }

    #[test]
    fn banner_uses_console_styles_and_resets_after_output() {
        let mut buffer = StyledBuffer::default();
        write_banner(&mut buffer).unwrap();
        let mut yellow = Style::new();
        yellow.text(Color::Yellow).intense(true);
        let mut red = Style::new();
        red.text(Color::Red).intense(true);
        assert_eq!(buffer.styles, vec![(0, yellow), (TITLE.len(), red),
            (TITLE.len() + SUBTITLE.len(), Style::new())]);
        assert!(!buffer.bytes.contains(&0x1b));
    }

    #[test]
    fn banner_resets_style_after_write_failure() {
        let mut buffer = StyledBuffer { fail: true, ..Default::default() };
        assert!(write_banner(&mut buffer).unwrap_err().to_string().contains("injected failure"));
        assert_eq!(buffer.styles.last().unwrap().1, Style::new());
    }
}
