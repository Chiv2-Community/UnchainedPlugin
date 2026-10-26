use log::{Level, Record};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::encode::{pattern::PatternEncoder, Color, Encode, Style, Write};

#[derive(Debug)]
struct ConsoleEncoder {
    header: PatternEncoder,
    body: PatternEncoder,
}

impl Default for ConsoleEncoder {
    fn default() -> Self {
        Self {
            header: PatternEncoder::new("[{d(%H:%M:%S)} {l:5}]"),
            body: PatternEncoder::new(" [{t}] {m}{n}"),
        }
    }
}

impl Encode for ConsoleEncoder {
    fn encode(&self, writer: &mut dyn Write, record: &Record) -> anyhow::Result<()> {
        let mut style = Style::new();
        match record.level() {
            Level::Error => {
                style.text(Color::Red).intense(true);
            }
            Level::Warn => {
                style.text(Color::Yellow).intense(true);
            }
            Level::Info => {
                style.text(Color::Blue).intense(true);
            }
            // Non-intense white is light gray; intense black is dark gray.
            Level::Debug => {
                style.text(Color::White).intense(false);
            }
            Level::Trace => {
                style.text(Color::Black).intense(true);
            }
        }
        writer.set_style(&style)?;
        let result = (|| {
            self.header.encode(writer, record)?;
            if record.level() <= Level::Info {
                writer.set_style(&Style::new())?;
            }
            self.body.encode(writer, record)
        })();
        // Do not leave later output colored, including after an encoding error.
        let reset = writer.set_style(&Style::new());
        result?;
        reset?;
        Ok(())
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleEncoderConfig {}

pub struct ConsoleEncoderDeserializer;

impl Deserialize for ConsoleEncoderDeserializer {
    type Trait = dyn Encode;
    type Config = ConsoleEncoderConfig;

    fn deserialize(
        &self,
        _: ConsoleEncoderConfig,
        _: &Deserializers,
    ) -> anyhow::Result<Box<dyn Encode>> {
        Ok(Box::new(ConsoleEncoder::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[derive(Default)]
    struct StyledBuffer {
        bytes: Vec<u8>,
        styles: Vec<(usize, Style)>,
    }

    impl io::Write for StyledBuffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Write for StyledBuffer {
        fn set_style(&mut self, style: &Style) -> io::Result<()> {
            self.styles.push((self.bytes.len(), style.clone()));
            Ok(())
        }
    }

    #[test]
    fn level_colors_and_debug_trace_body_style() {
        for (level, color, intense) in [
            (Level::Error, Color::Red, true),
            (Level::Warn, Color::Yellow, true),
            (Level::Info, Color::Blue, true),
            (Level::Debug, Color::White, false),
            (Level::Trace, Color::Black, true),
        ] {
            let mut buffer = StyledBuffer::default();
            ConsoleEncoder::default()
                .encode(
                    &mut buffer,
                    &Record::builder()
                        .level(level)
                        .target("plugin::module")
                        .args(format_args!("message"))
                        .build(),
                )
                .unwrap();
            let output = String::from_utf8(buffer.bytes.clone()).unwrap();
            assert!(output.contains(&format!("{level:5}] [plugin::module] message")));
            assert_eq!(buffer.styles[0].1.text, Some(color));
            assert_eq!(buffer.styles[0].1.intense, Some(intense));
            let (reset_at, reset_style) = &buffer.styles[1];
            assert_eq!(*reset_style, Style::new());
            if level >= Level::Debug {
                assert_eq!(*reset_at, buffer.bytes.len(), "debug and trace bodies must retain their gray style");
            } else {
                assert!(*reset_at < output.find("message").unwrap());
            }
            assert_eq!(buffer.styles.last().unwrap().1, Style::new());
        }
    }

    #[test]
    fn debug_preserves_embedded_message_colors() {
        let mut bytes = Vec::new();
        ConsoleEncoder::default()
            .encode(
                &mut log4rs::encode::writer::ansi::AnsiWriter(&mut bytes),
                &Record::builder()
                    .level(Level::Debug)
                    .target("plugin::module")
                    .args(format_args!("before \x1b[31mred\x1b[0m after"))
                    .build(),
            )
            .unwrap();
        let output = String::from_utf8(bytes).unwrap();
        assert!(output.starts_with("\x1b[0;37;22m"));
        assert!(output.contains("before \x1b[31mred\x1b[0m after"));
        assert!(output.ends_with("\x1b[0m"));
    }

    #[test]
    fn trace_uses_dark_gray_until_the_end_of_the_line() {
        let mut bytes = Vec::new();
        ConsoleEncoder::default()
            .encode(
                &mut log4rs::encode::writer::ansi::AnsiWriter(&mut bytes),
                &Record::builder()
                    .level(Level::Trace)
                    .target("plugin::module")
                    .args(format_args!("message"))
                    .build(),
            )
            .unwrap();
        let output = String::from_utf8(bytes).unwrap();
        let body = output.strip_prefix("\x1b[0;30;1m").unwrap()
            .strip_suffix("\x1b[0m").unwrap();
        assert!(body.contains("TRACE] [plugin::module] message"));
        assert!(!body.contains('\x1b'), "trace must not reset before the body");
    }

    #[test]
    fn unstyled_output_has_no_escape_sequences() {
        for level in [Level::Debug, Level::Trace] {
            let mut bytes = Vec::new();
            ConsoleEncoder::default()
                .encode(
                    &mut log4rs::encode::writer::simple::SimpleWriter(&mut bytes),
                    &Record::builder()
                        .level(level)
                        .target("plugin::module")
                        .args(format_args!("message"))
                        .build(),
                )
                .unwrap();
            let output = String::from_utf8(bytes).unwrap();
            assert!(output.contains("[plugin::module] message"));
            assert!(!output.contains('\x1b'));
        }
    }
}
