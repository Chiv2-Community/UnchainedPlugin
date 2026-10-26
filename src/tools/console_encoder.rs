use log::{Level, Record};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::encode::{pattern::PatternEncoder, Color, Encode, Style, Write};

#[cfg(windows)]
#[path = "console_palette.rs"]
mod windows_backend;

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
            ansi_colors: configure_console_colors(),
        }
    }
}

fn severity_rgb(level: Level) -> [u8; 3] {
    match level {
        Level::Error => [242, 139, 130],
        Level::Warn => [242, 204, 120],
        Level::Info => [140, 180, 225],
        Level::Debug => [128, 128, 128],
        Level::Trace => [92, 92, 92],
    }
}

#[cfg(any(windows, test))]
fn fallback_palette(mut colors: [u32; 16], attributes: u16) -> ([u32; 16], u16) {
    // Preserve normal message text in a separate slot before assigning DEBUG's gray.
    colors[15] = colors[(attributes & 0x0f) as usize];
    for (level, slot) in [(Level::Error, 12), (Level::Warn, 14), (Level::Info, 9),
        (Level::Debug, 7), (Level::Trace, 8)] {
        let [r, g, b] = severity_rgb(level);
        colors[slot] = u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16);
    }
    (colors, (attributes & !0x0f) | 15)
}

#[cfg(any(windows, test))]
fn use_truecolor(vt_enabled: bool, wine: bool, windows_terminal: bool) -> bool {
    vt_enabled && (!wine || windows_terminal)
}

fn configure_console_colors() -> bool {
    #[cfg(windows)]
    {
        windows_backend::configure()
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
            Level::Debug => {
                style.text(Color::White).intense(false);
            }
            Level::Trace => {
                style.text(Color::Black).intense(true);
            }
        }
        writer.set_style(&style)?;
        let result = (|| {
            if self.ansi_colors {
                let [r, g, b] = severity_rgb(record.level());
                write!(writer, "\x1b[0;38;2;{r};{g};{b}m")?;
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
        ConsoleEncoder {
            header: PatternEncoder::new("[{d(%H:%M:%S)} {l:5}]"),
            body: PatternEncoder::new(" [{M}] {m}{n}"),
            ansi_colors: false,
        }
    }

    #[test]
    fn truecolor_requires_vt_and_avoids_wine_graphical_console() {
        assert!(use_truecolor(true, false, false));
        assert!(use_truecolor(true, true, true));
        assert!(!use_truecolor(true, true, false));
        assert!(!use_truecolor(false, false, true));
    }

    #[test]
    fn fallback_palette_preserves_normal_text_background_and_unrelated_slots() {
        let original = std::array::from_fn(|i| i as u32 * 0x10101);
        let (colors, attributes) = fallback_palette(original, 0x27);
        assert_eq!(attributes, 0x2f);
        assert_eq!(colors[15], original[7]);
        assert_eq!(colors[7], 0x808080);
        assert_eq!(colors[8], 0x5c5c5c);
        assert_eq!(colors[9], 0xe1b48c);
        assert_eq!(colors[12], 0x828bf2);
        assert_eq!(colors[14], 0x78ccf2);
        for index in [0, 1, 2, 3, 4, 5, 6, 10, 11, 13] {
            assert_eq!(colors[index], original[index]);
        }
        assert_eq!(fallback_palette(colors, attributes), (colors, attributes));
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
            (Level::Debug, Color::White, false),
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
        assert!(output.starts_with("\x1b[0;37;22m"));
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
            (Level::Error, "242;139;130"),
            (Level::Warn, "242;204;120"),
            (Level::Info, "140;180;225"),
            (Level::Debug, "128;128;128"),
            (Level::Trace, "92;92;92"),
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
