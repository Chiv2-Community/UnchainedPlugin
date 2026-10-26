use log::{Level, Record};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::append::{console::{ConsoleAppender, Target}, Append};
use log4rs::encode::{writer::console::ConsoleWriter, EncoderConfig};
use log4rs::encode::{pattern::PatternEncoder, Color, Encode, Style, Write};

#[cfg(windows)]
#[path = "console_palette.rs"]
mod windows_backend;

#[derive(Debug)]
struct ConsoleEncoder {
    header: PatternEncoder,
    body: PatternEncoder,
    truecolor: bool,
}

impl Default for ConsoleEncoder {
    fn default() -> Self {
        Self {
            header: PatternEncoder::new("[{d(%H:%M:%S)} {l:5}]"),
            body: PatternEncoder::new(" {m}{n}"),
            truecolor: false,
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

struct ShortModule<'a>(&'a str);

impl std::fmt::Display for ShortModule<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut remaining = 16;
        let mut write_char = |ch| -> std::fmt::Result {
            if remaining > 0 {
                write!(f, "{ch}")?;
                remaining -= 1;
            }
            Ok(())
        };
        let mut segments = self.0.split("::").peekable();
        while let Some(segment) = segments.next() {
            if segments.peek().is_some() {
                for word in segment.split('_') {
                    let mut chars = word.chars().peekable();
                    let mut previous_uppercase = false;
                    let mut first = true;
                    while let Some(ch) = chars.next() {
                        let uppercase = ch.is_uppercase();
                        // Start camel-case words and acronym-to-word transitions.
                        if first || (uppercase && (!previous_uppercase
                            || chars.peek().is_some_and(|next| next.is_lowercase()))) {
                            write_char(ch)?;
                        }
                        first = false;
                        previous_uppercase = uppercase;
                    }
                }
                write_char(':')?;
                write_char(':')?;
            } else {
                for ch in segment.chars() { write_char(ch)?; }
            }
        }
        Ok(())
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

fn configure_console_colors(target: Target) -> bool {
    // Use the appender's own policy, including NO_COLOR and CLICOLOR overrides.
    let writer = match target {
        Target::Stdout => ConsoleWriter::stdout(),
        Target::Stderr => ConsoleWriter::stderr(),
    };
    if writer.is_none() { return false; }
    #[cfg(windows)]
    {
        windows_backend::configure(target)
    }
    #[cfg(not(windows))]
    {
        true
    }
}

impl Encode for ConsoleEncoder {
    fn encode(&self, writer: &mut dyn Write, record: &Record) -> anyhow::Result<()> {
        let (color, intense) = match record.level() {
            Level::Error => (Color::Red, true),
            Level::Warn => (Color::Yellow, true),
            Level::Info => (Color::Blue, true),
            Level::Debug => (Color::White, false),
            Level::Trace => (Color::Black, true),
        };
        let mut style = Style::new();
        style.text(color).intense(intense);
        writer.set_style(&style)?;
        let result = (|| {
            if self.truecolor {
                let [r, g, b] = severity_rgb(record.level());
                write!(writer, "\x1b[0;38;2;{r};{g};{b}m")?;
            }
            self.header.encode(writer, record)?;
            if record.level() <= Level::Info {
                writer.set_style(&Style::new())?;
            }
            write!(writer, " [{}]", ShortModule(record.module_path().unwrap_or("???")))?;
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum ConsoleTarget {
    Stdout,
    Stderr,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleAppenderConfig {
    target: Option<ConsoleTarget>,
    encoder: Option<EncoderConfig>,
    tty_only: Option<bool>,
}

pub struct ConsoleAppenderDeserializer;

impl Deserialize for ConsoleAppenderDeserializer {
    type Trait = dyn Append;
    type Config = ConsoleAppenderConfig;

    fn deserialize(&self, config: ConsoleAppenderConfig, deserializers: &Deserializers)
        -> anyhow::Result<Box<dyn Append>> {
        let target = match config.target.unwrap_or(ConsoleTarget::Stdout) {
            ConsoleTarget::Stdout => Target::Stdout,
            ConsoleTarget::Stderr => Target::Stderr,
        };
        let mut appender = ConsoleAppender::builder().target(target);
        if let Some(tty_only) = config.tty_only { appender = appender.tty_only(tty_only); }
        if let Some(encoder) = config.encoder {
            let encoder = if encoder.kind == "module_console" {
                let _: ConsoleEncoderConfig = encoder.config.deserialize_into()?;
                Box::new(ConsoleEncoder {
                    truecolor: configure_console_colors(target),
                    ..ConsoleEncoder::default()
                }) as Box<dyn Encode>
            } else {
                deserializers.deserialize(&encoder.kind, encoder.config)?
            };
            appender = appender.encoder(encoder);
        }
        Ok(Box::new(appender.build()))
    }
}

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
            body: PatternEncoder::new(" {m}{n}"),
            truecolor: false,
        }
    }

    #[test]
    fn module_abbreviation_preserves_leaf_unicode_and_file_records() {
        use log4rs::encode::writer::simple::SimpleWriter;

        for (module, expected) in [
            (Some("UnchainedPlugin::resolvers::messages"), "UP::r::messages"),
            (Some("UnchainedPlugin::modules::discord::event_log"), "UP::m::d::event_"),
            (Some("foo_bar::UnchainedPlugin::messages"), "fb::UP::messages"),
            (Some("fooBar::HTTPServer::XMLParser::messages"), "fB::HS::XP::mess"),
            (Some("abcdefghijklmnopq"), "abcdefghijklmnop"),
            (Some("ééééééééééééééééé"), "éééééééééééééééé"),
            (Some("_foo__bar_::HTTP::messages"), "fb::H::messages"),
            (Some("ÉquipeMonde::équipe_monde::messages"), "ÉM::ém::messages"),
            (Some("single"), "single"),
            (Some(""), ""),
            (Some("équipe::世界::消息"), "é::世::消息"),
            (None, "???"),
        ] {
            let record = Record::builder().level(Level::Info).module_path(module)
                .target("unchanged::filter_target").args(format_args!("message")).build();
            let mut console = Vec::new();
            encoder().encode(&mut SimpleWriter(&mut console), &record).unwrap();
            assert!(String::from_utf8(console).unwrap().contains(&format!("[{expected}] message")));
            assert!(expected.chars().count() <= 16);
            let mut file = Vec::new();
            PatternEncoder::new("{M}").encode(&mut SimpleWriter(&mut file), &record).unwrap();
            assert_eq!(String::from_utf8(file).unwrap(), module.unwrap_or("???"));
            assert_eq!(record.target(), "unchanged::filter_target");
            assert_eq!(record.module_path(), module);
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
        fail_after: Option<usize>,
    }

    impl io::Write for StyledBuffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if let Some(limit) = self.fail_after {
                let remaining = limit.saturating_sub(self.bytes.len());
                if remaining == 0 {
                    return Err(io::Error::other("injected output failure"));
                }
                let count = remaining.min(bytes.len());
                self.bytes.extend_from_slice(&bytes[..count]);
                return Ok(count);
            }
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
    fn encoding_errors_reset_style_and_preserve_the_original_error() {
        for level in [Level::Info, Level::Debug, Level::Trace] {
            for truecolor in [false, true] {
                for fail_after in [0, 25] {
                    let mut buffer = StyledBuffer { fail_after: Some(fail_after), ..Default::default() };
                    let mut encoder = encoder();
                    encoder.truecolor = truecolor;
                    let error = encoder.encode(
                        &mut buffer,
                        &Record::builder().level(level).module_path(Some("plugin::module"))
                            .args(format_args!("message")).build(),
                    ).unwrap_err();
                    assert!(error.to_string().contains("injected output failure"));
                    assert_eq!(buffer.styles.last().unwrap().1, Style::new());
                }
            }
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
            assert!(output.contains(&format!("{level:5}] [p::module] message")));
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
        assert!(output.contains("INFO ] [p::module] message"));
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
        assert!(body.contains("TRACE] [p::module] message"));
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
            assert!(output.contains("[p::module] message"));
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
            encoder.truecolor = true;
            encoder.encode(
                &mut log4rs::encode::writer::ansi::AnsiWriter(&mut bytes),
                &Record::builder().level(level).module_path(Some("plugin::module"))
                    .args(format_args!("before \x1b[31mred\x1b[0m after")).build(),
            ).unwrap();
            let output = String::from_utf8(bytes).unwrap();
            let header = output.find(&format!("\x1b[0;38;2;{rgb}m")).unwrap();
            let body = output.find(" [p::module]").unwrap();
            assert!(header < body);
            assert_eq!(output[header..body].contains("\x1b[0m"), level <= Level::Info);
            assert!(output.contains("before \x1b[31mred\x1b[0m after"));
            assert!(output.ends_with("\x1b[0m"));
        }
    }
}
