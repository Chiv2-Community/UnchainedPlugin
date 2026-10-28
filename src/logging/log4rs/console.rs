use log4rs::append::{console::{ConsoleAppender, Target}, Append};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::encode::{Encode, EncoderConfig};

use crate::logging::console::{configure_console_colors, ConsoleEncoder};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleEncoderConfig {
    header_pattern: Option<String>,
    body_pattern: Option<String>,
}

impl ConsoleEncoderConfig {
    fn build(self) -> ConsoleEncoder {
        ConsoleEncoder::with_patterns(
            self.header_pattern.as_deref(),
            self.body_pattern.as_deref(),
        )
    }
}

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
                let config: ConsoleEncoderConfig = encoder.config.deserialize_into()?;
                let mut encoder = config.build();
                encoder.truecolor = configure_console_colors(target);
                Box::new(encoder) as Box<dyn Encode>
            } else {
                deserializers.deserialize(&encoder.kind, encoder.config)?
            };
            appender = appender.encoder(encoder);
        }
        Ok(Box::new(appender.build()))
    }
}

pub struct ConsoleEncoderDeserializer;

impl Deserialize for ConsoleEncoderDeserializer {
    type Trait = dyn Encode;
    type Config = ConsoleEncoderConfig;

    fn deserialize(&self, config: ConsoleEncoderConfig, _: &Deserializers)
        -> anyhow::Result<Box<dyn Encode>> {
        Ok(Box::new(config.build()))
    }
}
