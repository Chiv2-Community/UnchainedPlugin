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
                if !config.tty_only.unwrap_or(false) || writer.is_some() {
                    let appender = queued_console::QueuedConsole::new(encoder, target, writer)?;
                    crate::logging::console::enable_selection();
                    return Ok(Box::new(appender));
                }
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

    #[cfg(windows)]
    #[test_case::test_case("NO_COLOR", true; "no_color_tty_only")]
    #[test_case::test_case("NO_COLOR", false; "no_color_raw")]
    #[test_case::test_case("CLICOLOR", true; "clicolor_zero_tty_only")]
    #[test_case::test_case("CLICOLOR", false; "clicolor_zero_raw")]
    #[test_case::test_case("CLICOLOR_FORCE", true; "forced_color_tty_only")]
    #[test_case::test_case("CLICOLOR_FORCE", false; "forced_color")]
    #[ignore = "opens native console windows; run explicitly on an interactive desktop"]
    fn owned_console_preserves_builtin_tty_only(mode: &str, tty_only: bool) {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "--nocapture",
                &format!("{}::tty_only_child", module_path!().split_once("::").unwrap().1)])
            .env_remove("NO_COLOR").env_remove("CLICOLOR").env_remove("CLICOLOR_FORCE")
            .env(mode, if mode == "CLICOLOR" { "0" } else { "1" })
            .env("UNCHAINED_TEST_TTY_ONLY", tty_only.to_string())
            .creation_flags(0x08000000).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "subprocess fixture for owned console tty_only"]
    fn tty_only_child() {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::Console::{FreeConsole, GetStdHandle, SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
        use log4rs::encode::writer::console::ConsoleWriter;
        unsafe {
            let stderr = GetStdHandle(STD_ERROR_HANDLE).unwrap();
            let _ = FreeConsole();
            crate::logging::console::spawn_console();
            SetStdHandle(STD_ERROR_HANDLE, stderr).unwrap();
            let output = std::fs::OpenOptions::new().read(true).write(true).open("CONOUT$").unwrap();
            SetStdHandle(STD_OUTPUT_HANDLE, HANDLE(output.as_raw_handle() as isize)).unwrap();
            assert!(crate::logging::console::owns_console());
            assert!(crate::logging::console::is_console(Target::Stdout));
            let tty_only: bool = std::env::var("UNCHAINED_TEST_TTY_ONLY").unwrap().parse().unwrap();
            let has_writer = ConsoleWriter::stdout().is_some();
            assert_eq!(has_writer, std::env::var_os("CLICOLOR_FORCE").is_some());
            let builtin = ConsoleAppender::builder().tty_only(tty_only).build();
            let appender = ConsoleAppenderDeserializer.deserialize(ConsoleAppenderConfig {
                target: None, encoder: None, tty_only: Some(tty_only),
            }, &Deserializers::default()).unwrap();
            struct Probe(std::cell::Cell<usize>);
            impl std::fmt::Display for Probe {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    self.0.set(self.0.get() + 1);
                    formatter.write_str("tty probe")
                }
            }
            let probe = Probe(std::cell::Cell::new(0));
            builtin.append(&log::Record::builder().args(format_args!("{probe}")).build()).unwrap();
            let expected = probe.0.replace(0);
            assert_eq!(expected, usize::from(!tty_only || has_writer));
            appender.append(&log::Record::builder().args(format_args!("{probe}")).build()).unwrap();
            assert_eq!(probe.0.get(), expected);
        }
    }

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
