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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows::Win32::System::Console::*;

    #[test]
    fn selection_is_enabled_only_for_owned_queued_consoles_with_or_without_colors() {
        for color in [false, true] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.args(["--ignored", "--exact", "--nocapture",
                "logging::log4rs::console::tests::selection_mode_child"])
                .env_remove("NO_COLOR").env_remove("CLICOLOR").env_remove("CLICOLOR_FORCE");
            if color { child.env("CLICOLOR_FORCE", "1"); }
            else { child.env("NO_COLOR", "1"); }
            let output = child.output().unwrap();
            assert!(output.status.success(), "color={color}: {}\n{}",
                String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        }
    }

    #[test]
    #[ignore = "subprocess fixture for console selection mode"]
    fn selection_mode_child() {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        let input_handle = || {
            let file = std::fs::OpenOptions::new().read(true).write(true).open("CONIN$").unwrap();
            unsafe { SetStdHandle(STD_INPUT_HANDLE, HANDLE(file.as_raw_handle() as isize)).unwrap(); }
            file
        };
        let build = || ConsoleAppenderDeserializer.deserialize(ConsoleAppenderConfig {
            target: None, encoder: None, tty_only: None,
        }, &Deserializers::default()).unwrap();
        let mode = || unsafe {
            let mut mode = CONSOLE_MODE(0);
            GetConsoleMode(GetStdHandle(STD_INPUT_HANDLE).unwrap(), &mut mode).unwrap();
            mode
        };
        unsafe {
            let _ = FreeConsole();
            AllocConsole().unwrap();
        }
        let shared_input = input_handle();
        unsafe {
            let input = GetStdHandle(STD_INPUT_HANDLE).unwrap();
            SetConsoleMode(input, (mode() | ENABLE_EXTENDED_FLAGS) & !ENABLE_QUICK_EDIT_MODE).unwrap();
        }
        assert!(!crate::logging::console::owns_console());
        let before = mode();
        drop(build());
        assert_eq!(mode(), before, "shared consoles must remain unchanged");
        unsafe { FreeConsole().unwrap(); }
        drop(shared_input);
        crate::logging::console::spawn_console();
        assert!(crate::logging::console::owns_console());
        let _owned_input = input_handle();
        let owned_output = std::fs::OpenOptions::new().read(true).write(true).open("CONOUT$").unwrap();
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, HANDLE(owned_output.as_raw_handle() as isize)).unwrap();
            SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE).unwrap(),
                (mode() | ENABLE_EXTENDED_FLAGS) & !ENABLE_QUICK_EDIT_MODE).unwrap();
        }
        assert!(!mode().contains(ENABLE_QUICK_EDIT_MODE));
        let appender = build();
        assert!(format!("{appender:?}").starts_with("QueuedConsole"));
        drop(appender);
        assert!(mode().contains(ENABLE_QUICK_EDIT_MODE | ENABLE_EXTENDED_FLAGS));
        assert!(!mode().contains(ENABLE_MOUSE_INPUT));
    }
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
        #[cfg(windows)]
        {
            crate::logging::console::preserve_console_handles();
            if crate::logging::console::owns_console() {
                use log4rs::encode::writer::console::ConsoleWriter;
                let writer = match target {
                    Target::Stdout => ConsoleWriter::stdout(),
                    Target::Stderr => ConsoleWriter::stderr(),
                };
                if writer.is_some() || !config.tty_only.unwrap_or(false) {
                    let appender = queued_console::QueuedConsole::new(encoder, target, writer)?;
                    crate::logging::console::enable_selection();
                    return Ok(Box::new(appender));
                }
                crate::logging::console::enable_selection();
            }
        }
        Ok(Box::new(ConsoleAppender::builder().target(target)
            .tty_only(config.tty_only.unwrap_or(false)).encoder(encoder).build()))
    }
}
