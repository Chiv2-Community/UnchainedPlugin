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
    fn routing_and_selection_respect_console_ownership_target_and_color_policy() {
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
        let build = |target, tty_only| ConsoleAppenderDeserializer.deserialize(
            serde_yaml::from_str(&format!("target: {target}\ntty_only: {tty_only}\nencoder:\n  kind: palette\n  pattern: '{{m}}'\n")).unwrap(),
            &Deserializers::default()).unwrap();
        let mode = || unsafe {
            let input = std::fs::OpenOptions::new().read(true).write(true).open("CONIN$").unwrap();
            let mut mode = CONSOLE_MODE(0);
            GetConsoleMode(HANDLE(input.as_raw_handle() as isize), &mut mode).unwrap();
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
        assert!(!format!("{:?}", build("stdout", false)).starts_with("QueuedConsole"));
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
        let redirected_input = std::fs::File::open("NUL").unwrap();
        let redirected_handle = HANDLE(redirected_input.as_raw_handle() as isize);
        unsafe { SetStdHandle(STD_INPUT_HANDLE, redirected_handle).unwrap(); }
        let appender = build("stdout", false);
        assert!(format!("{appender:?}").starts_with("QueuedConsole"));
        drop(appender);
        assert!(mode().contains(ENABLE_QUICK_EDIT_MODE | ENABLE_EXTENDED_FLAGS));
        assert!(!mode().contains(ENABLE_MOUSE_INPUT));
        assert_eq!(unsafe { GetStdHandle(STD_INPUT_HANDLE).unwrap() }, redirected_handle);
        // Wine reports QuickEdit in GetConsoleMode even when its renderer ignores
        // mouse selection. Check the actual highlight, not just the input flags.
        unsafe {
            use windows::core::{s, w};
            use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
            if GetProcAddress(GetModuleHandleW(w!("ntdll.dll")).unwrap(), s!("wine_get_version")).is_some() {
                use windows::Win32::Foundation::{LPARAM, WPARAM};
                use windows::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC};
                use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_KEYDOWN};
                let window = GetConsoleWindow();
                let dc = GetDC(window);
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                let highlighted = loop {
                    // Initial window paint can race with the first selection.
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let before = GetPixel(dc, 4, 4);
                    SendMessageW(window, WM_LBUTTONDOWN, WPARAM(1), LPARAM((4 << 16) | 4));
                    SendMessageW(window, WM_MOUSEMOVE, WPARAM(1), LPARAM((4 << 16) | 64));
                    SendMessageW(window, WM_LBUTTONUP, WPARAM(0), LPARAM((4 << 16) | 64));
                    let selected = GetPixel(dc, 4, 4);
                    SendMessageW(window, WM_KEYDOWN, WPARAM(0x1b), LPARAM(0));
                    if before != selected { break true; }
                    if std::time::Instant::now() >= deadline { break false; }
                };
                ReleaseDC(window, dc);
                assert!(highlighted, "mouse drag must visibly highlight console cells");
            }
        }
        let path = std::env::temp_dir().join(format!("unchained-console-routing-{}", std::process::id()));
        let message = "x".repeat(70 * 1024);
        for (target, handle) in [("stdout", STD_OUTPUT_HANDLE), ("stderr", STD_ERROR_HANDLE)] {
            let redirected = std::fs::File::create(&path).unwrap();
            unsafe { SetStdHandle(handle, HANDLE(owned_output.as_raw_handle() as isize)).unwrap(); }
            for tty_only in [false, true] {
                assert!(format!("{:?}", build(target, tty_only)).starts_with("QueuedConsole"));
            }
            unsafe { SetStdHandle(handle, HANDLE(redirected.as_raw_handle() as isize)).unwrap(); }
            for tty_only in [false, true] {
                redirected.set_len(0).unwrap();
                let appender = build(target, tty_only);
                assert!(!format!("{appender:?}").starts_with("QueuedConsole"));
                appender.append(&log::Record::builder().args(format_args!("{message}")).build()).unwrap();
                appender.flush();
                assert_eq!(std::fs::read(&path).unwrap(), if tty_only { b"" } else { message.as_bytes() });
            }
            unsafe { SetStdHandle(handle, HANDLE(owned_output.as_raw_handle() as isize)).unwrap(); }
        }
        std::fs::remove_file(path).unwrap();
        crate::logging::console::attach_stdio();
        for target in ["stdout", "stderr"] {
            assert!(!format!("{:?}", build(target, false)).starts_with("QueuedConsole"));
        }
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
