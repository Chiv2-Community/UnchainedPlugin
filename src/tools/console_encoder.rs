use log::{Level, Record};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::encode::{pattern::PatternEncoder, Color, Encode, Style, Write};

#[derive(Debug)]
struct ConsoleEncoder {
    header: PatternEncoder,
    body: PatternEncoder,
    ansi_colors: bool,
}

impl Default for ConsoleEncoder {
    fn default() -> Self {
        Self {
            header: PatternEncoder::new("[{d(%H:%M:%S)} {l:5}]"),
            body: PatternEncoder::new(" [{M}] {m}{n}"),
            ansi_colors: supports_ansi_colors(),
        }
    }
}

fn supports_ansi_colors() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Console::{GetConsoleMode, GetStdHandle, SetConsoleMode,
            CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE};
        let Ok(handle) = GetStdHandle(STD_OUTPUT_HANDLE) else { return false };
        let mut mode = CONSOLE_MODE(0);
        GetConsoleMode(handle, &mut mode).is_ok()
            && SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING).is_ok()
    }
    #[cfg(not(windows))]
    {
        use std::io::IsTerminal;
        std::io::stdout().is_terminal()
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
            // Legacy consoles only provide one usable dark gray.
            Level::Debug => {
                style.text(Color::Black).intense(true);
            }
            Level::Trace => {
                style.text(Color::Black).intense(true);
            }
        }
        writer.set_style(&style)?;
        let result = (|| {
            if self.ansi_colors {
                let color = match record.level() {
                    Level::Error => "239;154;154",
                    Level::Warn => "230;202;139",
                    Level::Info => "154;177;204",
                    Level::Debug => "128;128;128",
                    Level::Trace => "104;104;104",
                };
                write!(writer, "\x1b[0;38;2;{color}m")?;
            }
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

    fn encoder() -> ConsoleEncoder {
        ConsoleEncoder { ansi_colors: false, ..ConsoleEncoder::default() }
    }

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
            (Level::Debug, Color::Black, true),
            (Level::Trace, Color::Black, true),
        ] {
            let mut buffer = StyledBuffer::default();
            encoder()
                .encode(
                    &mut buffer,
                    &Record::builder()
                        .level(level)
                        .target("plugin::module")
                        .module_path(Some("plugin::module"))
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
    fn console_uses_source_module_instead_of_target() {
        let mut bytes = Vec::new();
        encoder()
            .encode(
                &mut log4rs::encode::writer::simple::SimpleWriter(&mut bytes),
                &Record::builder()
                    .level(Level::Info)
                    .target("plugin::module::game_chat")
                    .module_path(Some("plugin::module"))
                    .args(format_args!("message"))
                    .build(),
            )
            .unwrap();
        let output = String::from_utf8(bytes).unwrap();
        assert!(output.contains("INFO ] [plugin::module] message"));
        assert!(!output.contains("game_chat"));
    }

    #[test]
    fn debug_preserves_embedded_message_colors() {
        let mut bytes = Vec::new();
        encoder()
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
        assert!(output.starts_with("\x1b[0;30;1m"));
        assert!(output.contains("before \x1b[31mred\x1b[0m after"));
        assert!(output.ends_with("\x1b[0m"));
    }

    #[test]
    fn trace_uses_dark_gray_until_the_end_of_the_line() {
        let mut bytes = Vec::new();
        encoder()
            .encode(
                &mut log4rs::encode::writer::ansi::AnsiWriter(&mut bytes),
                &Record::builder()
                    .level(Level::Trace)
                    .target("plugin::module")
                    .module_path(Some("plugin::module"))
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
            encoder()
                .encode(
                    &mut log4rs::encode::writer::simple::SimpleWriter(&mut bytes),
                    &Record::builder()
                        .level(level)
                        .target("plugin::module")
                        .module_path(Some("plugin::module"))
                        .args(format_args!("message"))
                        .build(),
                )
                .unwrap();
            let output = String::from_utf8(bytes).unwrap();
            assert!(output.contains("[plugin::module] message"));
            assert!(!output.contains('\x1b'));
        }
    }

    #[test]
    fn pastel_headers_and_darker_gray_bodies_use_rgb() {
        for (level, rgb) in [
            (Level::Error, "239;154;154"),
            (Level::Warn, "230;202;139"),
            (Level::Info, "154;177;204"),
            (Level::Debug, "128;128;128"),
            (Level::Trace, "104;104;104"),
        ] {
            let mut bytes = Vec::new();
            let mut encoder = encoder();
            encoder.ansi_colors = true;
            encoder.encode(
                &mut log4rs::encode::writer::ansi::AnsiWriter(&mut bytes),
                &Record::builder().level(level).module_path(Some("plugin::module"))
                    .args(format_args!("before \x1b[31mred\x1b[0m after")).build(),
            ).unwrap();
            let output = String::from_utf8(bytes).unwrap();
            let header = output.find(&format!("\x1b[0;38;2;{rgb}m")).unwrap();
            let body = output.find(" [plugin::module]").unwrap();
            assert!(header < body);
            assert_eq!(output[header..body].contains("\x1b[0m"), level <= Level::Info);
            assert!(output.contains("before \x1b[31mred\x1b[0m after"));
            assert!(output.ends_with("\x1b[0m"));
        }
    }
}
