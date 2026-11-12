use log4rs::append::{console::{ConsoleAppender, Target}, Append};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::encode::{Encode, EncoderConfig};

use super::palette::PaletteEncoderConfig;

#[cfg(windows)]
#[path = "queued_console.rs"]
mod queued_console;

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum ConsoleTarget { Stdout, Stderr }

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleAppenderConfig {
    target: Option<ConsoleTarget>,
    encoder: Option<EncoderConfig>,
    tty_only: Option<bool>,
}

pub struct ConsoleAppenderDeserializer;

#[derive(Debug)]
struct EmojiFilter(Box<dyn Encode>);

impl Encode for EmojiFilter {
    fn encode(&self, writer: &mut dyn log4rs::encode::Write, record: &log::Record) -> anyhow::Result<()> {
        let mut line = super::removals::RenderedLine::default();
        self.0.encode(&mut line, record)?;
        line.emit_without_emojis(writer)
    }
}

impl Deserialize for ConsoleAppenderDeserializer {
    type Trait = dyn Append;
    type Config = ConsoleAppenderConfig;

    fn deserialize(&self, config: ConsoleAppenderConfig, deserializers: &Deserializers)
        -> anyhow::Result<Box<dyn Append>> {
        let target = match config.target.unwrap_or(ConsoleTarget::Stdout) {
            ConsoleTarget::Stdout => Target::Stdout,
            ConsoleTarget::Stderr => Target::Stderr,
        };
        let is_json = config.encoder.as_ref().is_some_and(|encoder| encoder.kind == "json");
        let encoder: Box<dyn Encode> = if let Some(encoder) = config.encoder {
            if encoder.kind == "palette" {
                let config: PaletteEncoderConfig = encoder.config.deserialize_into()?;
                let mut encoder = config.build()?;
                encoder.configure(target);
                Box::new(encoder) as Box<dyn Encode>
            } else {
                deserializers.deserialize(&encoder.kind, encoder.config)?
            }
        } else {
            Box::<log4rs::encode::pattern::PatternEncoder>::default()
        };
        let encoder = if !is_json && crate::logging::console::should_strip_console_emojis(target) {
            Box::new(EmojiFilter(encoder)) as Box<dyn Encode>
        } else { encoder };
        #[cfg(windows)]
        {
            crate::logging::console::preserve_console_handles();
            if crate::logging::console::owns_console() && crate::logging::console::is_console(target) {
                use log4rs::encode::writer::console::ConsoleWriter;
                let writer = match target {
                    Target::Stdout => ConsoleWriter::stdout(),
                    Target::Stderr => ConsoleWriter::stderr(),
                };
                let appender = queued_console::QueuedConsole::new(encoder, target, writer)?;
                crate::logging::console::enable_selection();
                return Ok(Box::new(appender));
            }
        }
        Ok(Box::new(ConsoleAppender::builder().target(target)
            .tty_only(config.tty_only.unwrap_or(false)).encoder(encoder).build()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log4rs::encode::writer::simple::SimpleWriter;

    #[test]
    fn emoji_filter_removes_emoji_from_fields_and_message() {
        let fields = [("sender", "Player 🎮")];
        let record = log::Record::builder().key_values(&fields)
            .args(format_args!("🛠️ Hello 👩‍💻 雪 🇩🇪\r\n")).build();
        let pattern = "{K(sender)}: {m}";
        let encoder = EmojiFilter(Box::new(super::super::pattern::PatternEncoder::new(pattern)));
        let mut output = Vec::new();
        encoder.encode(&mut SimpleWriter(&mut output), &record).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "Player :  Hello  雪 \r\n");

    }
}
