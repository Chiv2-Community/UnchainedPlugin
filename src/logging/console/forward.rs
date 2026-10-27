//! Opt-in console transport. Game threads only encode and try a bounded queue.
use std::{io::{self, Write},
    sync::{atomic::{AtomicU64, Ordering}, mpsc::{self, SyncSender, TrySendError}, OnceLock}};

use serde::Serialize;

const CAPACITY: usize = 256;
const MAX_TEXT: usize = 16 * 1024;
static FORWARDER: OnceLock<Option<Forwarder>> = OnceLock::new();

struct Forwarder {
    sender: SyncSender<Event>,
    dropped: AtomicU64,
}

#[derive(Serialize)]
struct Event {
    version: u8,
    kind: &'static str,
    level: Option<String>,
    text: String,
    dropped: u64,
}

fn handle_value(value: &str) -> Option<usize> {
    let handle = value.parse::<usize>().ok()?;
    (handle != 0 && handle != usize::MAX).then_some(handle)
}

#[cfg(windows)]
fn inherited_pipe(value: &str) -> Option<std::fs::File> {
    use std::os::windows::io::FromRawHandle;
    use windows::Win32::{Foundation::{HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation},
        Storage::FileSystem::{GetFileType, FILE_TYPE_PIPE}};
    let raw = handle_value(value)?;
    let handle = HANDLE(raw as isize);
    unsafe {
        if GetFileType(handle) != FILE_TYPE_PIPE { return None; }
        // This launch owns the handle; descendants must not keep the pipe alive.
        SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)).ok()?;
        Some(std::fs::File::from_raw_handle(raw as *mut _))
    }
}

#[cfg(all(not(windows), any(test, logging_test_harness)))]
fn inherited_pipe(_value: &str) -> Option<std::fs::File> {
    None // Native logging tests only; Proton uses the Windows implementation.
}

fn forwarder() -> Option<&'static Forwarder> {
    FORWARDER.get_or_init(|| {
        let mut stream = inherited_pipe(&std::env::var("UNCHAINED_CONSOLE_HANDLE").ok()?)?;
        let (sender, receiver) = mpsc::sync_channel(CAPACITY);
        std::thread::Builder::new().name("console-forward".into()).spawn(move || {
            // A failed or slow receiver cannot stall game threads. File logging remains active.
            let _ = (|| -> io::Result<()> {
                for event in receiver {
                    serde_json::to_writer(&mut stream, &event)?;
                    stream.write_all(b"\n")?;
                }
                Ok(())
            })();
        }).ok()?;
        Some(Forwarder { sender, dropped: AtomicU64::new(0) })
    }).as_ref()
}

pub(crate) fn enabled() -> bool { !super::attach_stdio_enabled() && forwarder().is_some() }

pub(crate) fn send(kind: &'static str, level: Option<log::Level>, text: String) {
    if super::attach_stdio_enabled() { return; }
    if let Some(forwarder) = forwarder() {
        forwarder.send(Event { version: 1, kind, level: level.map(|level| level.to_string()),
            text, dropped: 0 });
    }
}

impl Forwarder {
    fn send(&self, mut event: Event) {
        event.dropped = self.dropped.swap(0, Ordering::Relaxed);
        if let Err(TrySendError::Full(event) | TrySendError::Disconnected(event)) = self.sender.try_send(event) {
            self.dropped.fetch_add(event.dropped.saturating_add(1), Ordering::Relaxed);
        }
    }
}

// Bound encoding too: one pathological log message must not allocate an unbounded frame.
#[derive(Default)]
pub(crate) struct TextBuffer {
    bytes: Vec<u8>,
    truncated: bool,
}

impl Write for TextBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(MAX_TEXT.saturating_sub(self.bytes.len()));
        self.bytes.extend_from_slice(&bytes[..count]);
        self.truncated |= count < bytes.len();
        // Consume excess bytes without blocking or growing the buffer.
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl TextBuffer {
    pub(crate) fn finish(self) -> String {
        let mut text = String::from_utf8_lossy(&self.bytes).into_owned();
        if self.truncated {
            const MARKER: &str = " [truncated]";
            let mut end = MAX_TEXT - MARKER.len();
            while !text.is_char_boundary(end) { end -= 1; }
            text.truncate(end);
            text.push_str(MARKER);
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::time::Duration;

    #[test]
    fn handle_must_be_nonzero_unsigned_decimal() {
        assert_eq!(handle_value("1234"), Some(1234));
        for value in ["0", "-1", "pipe", "123\n", "", &usize::MAX.to_string()] {
            assert!(handle_value(value).is_none());
        }
    }

    #[cfg(windows)]
    #[test]
    fn non_pipe_handles_are_rejected_without_taking_ownership() {
        use std::os::windows::io::AsRawHandle;
        let file = std::fs::File::open(std::env::current_exe().unwrap()).unwrap();
        assert!(inherited_pipe(&(file.as_raw_handle() as usize).to_string()).is_none());
        assert!(file.metadata().unwrap().len() != 0);
        assert!(inherited_pipe("123456789").is_none());
    }

    #[test]
    fn saturated_queue_reports_loss_without_waiting() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let forwarder = Forwarder { sender, dropped: AtomicU64::new(0) };
        let event = || Event { version: 1, kind: "log", level: None, text: "message".into(), dropped: 0 };
        forwarder.send(event());
        forwarder.send(event());
        forwarder.send(event());
        assert_eq!(receiver.recv().unwrap().dropped, 0);
        forwarder.send(event());
        assert_eq!(receiver.recv().unwrap().dropped, 2);
    }

    #[test]
    fn oversized_unicode_output_is_bounded_and_valid_json() {
        let mut buffer = TextBuffer::default();
        write!(buffer, "{}", "世界".repeat(MAX_TEXT)).unwrap();
        let text = buffer.finish();
        assert!(text.len() <= MAX_TEXT);
        assert!(text.ends_with(" [truncated]"));
        let encoded = serde_json::to_string(&text).unwrap();
        assert_eq!(serde_json::from_str::<String>(&encoded).unwrap(), text);
    }

    #[cfg(windows)]
    #[test]
    fn actual_appender_and_banner_use_inherited_pipe() {
        check_inherited_pipe(false);
    }

    #[cfg(windows)]
    #[test]
    fn suppressing_console_creation_preserves_structured_forwarding() {
        check_inherited_pipe(true);
    }

    #[cfg(windows)]
    fn check_inherited_pipe(suppress_console: bool) {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let marker = std::env::temp_dir().join(format!("forward-test-{}-{unique}", std::process::id()));
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "--quiet", &format!("{}::forward_child", module_path!().split_once("::").unwrap().1)])
            .env("UNCHAINED_FORWARD_TEST_MARKER", &marker)
            .env("UNCHAINED_FORWARD_TEST_NO_CONSOLE", suppress_console.to_string())
            .stdout(std::process::Stdio::piped()).spawn().unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        let mut frames = Vec::new();
        // The Rust test harness also writes to stdout; only collect structured frames.
        while frames.len() < 3 {
            line.clear();
            assert_ne!(reader.read_line(&mut line).unwrap(), 0, "child ended before all frames");
            assert!(line.len() < 128 * 1024);
            if let Ok(frame) = serde_json::from_str::<serde_json::Value>(&line) { frames.push(frame); }
        }
        std::fs::write(&marker, "done").unwrap();
        assert!(child.wait().unwrap().success());
        std::fs::remove_file(marker).unwrap();
        assert_eq!(frames[0]["kind"], "log");
        assert_eq!(frames[0]["level"], "INFO");
        let text = frames[0]["text"].as_str().unwrap();
        assert!(text.contains("[UP::test                ] hello 世界\nsecond line"), "{text:?}");
        assert!(!text.contains('\x1b'));
        assert_eq!(frames[1]["level"], "DEBUG");
        assert_eq!(frames[2]["kind"], "banner");
        assert!(!frames[2]["text"].as_str().unwrap().contains('\x1b'));
    }

    #[test]
    #[ignore = "subprocess fixture for actual_appender_and_banner_use_inherited_pipe"]
    fn forward_child() {
        if std::env::var("UNCHAINED_FORWARD_TEST_NO_CONSOLE").as_deref() == Ok("true") {
            super::super::disable_console_spawning();
            assert!(!super::super::attach_stdio_enabled());
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            std::env::set_var("UNCHAINED_CONSOLE_HANDLE", (std::io::stdout().as_raw_handle() as usize).to_string());
        }
        if std::env::var_os("UNCHAINED_FORWARD_TEST_FLOOD").is_some() {
            assert!(enabled());
            for _ in 0..10000 { send("log", Some(log::Level::Info), "x".repeat(MAX_TEXT)); }
            assert!(forwarder().unwrap().dropped.load(Ordering::Relaxed) != 0);
            // Bypass the harness's final stdout write: that is deliberately backpressured.
            std::process::exit(0);
        }
        emit_console_records();
        let marker = std::env::var("UNCHAINED_FORWARD_TEST_MARKER").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !std::path::Path::new(&marker).exists() {
            assert!(std::time::Instant::now() < deadline, "receiver did not acknowledge test frames");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(windows)]
    #[test]
    fn full_or_broken_pipe_never_blocks_the_logging_thread() {
        for close_reader in [false, true] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", "--quiet", &format!("{}::forward_child", module_path!().split_once("::").unwrap().1)])
                .env("UNCHAINED_FORWARD_TEST_FLOOD", "1")
                .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().unwrap();
            if close_reader {
                let mut reader = BufReader::new(child.stdout.take().unwrap());
                reader.read_line(&mut String::new()).unwrap(); // Let the harness start before disconnecting.
                drop(reader);
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(status) = child.try_wait().unwrap() { assert!(status.success()); break; }
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("logging waited for the pipe reader");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    #[test]
    fn attach_stdio_preserves_pipes_and_overrides_ipc_without_allocating_a_console() {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args(["--ignored", "--exact", "--nocapture", &format!("{}::attach_stdio_child", module_path!().split_once("::").unwrap().1)])
            .env("UNCHAINED_CONSOLE_HANDLE", "1234");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW; stdout/stderr are supplied pipes.
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("hello 世界\nsecond line"), "{stdout}");
        assert!(stdout.contains("▄████████"));
        assert!(!stdout.contains('\x1b'));
    }

    #[test]
    #[ignore = "subprocess fixture for attach_stdio_preserves_pipes_and_overrides_ipc_without_allocating_a_console"]
    fn attach_stdio_child() {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            std::env::set_var("UNCHAINED_CONSOLE_HANDLE", (std::io::stdout().as_raw_handle() as usize).to_string());
        }
        #[cfg(windows)]
        let before = unsafe { windows::Win32::System::Console::GetConsoleWindow() };
        super::super::attach_stdio();
        assert!(!enabled());
        emit_console_records();
        #[cfg(windows)]
        assert_eq!(before, unsafe { windows::Win32::System::Console::GetConsoleWindow() });
    }

    #[test]
    fn suppressing_console_creation_preserves_standard_output_without_attachment() {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args(["--ignored", "--exact", "--nocapture", &format!("{}::no_spawn_console_child", module_path!().split_once("::").unwrap().1)])
            .env_remove("UNCHAINED_CONSOLE_HANDLE");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("hello 世界\nsecond line"), "{stdout}");
        assert!(stdout.contains("▄████████"));
        assert!(!stdout.contains('\x1b'));
    }

    #[test]
    #[ignore = "subprocess fixture for console suppression without attachment"]
    fn no_spawn_console_child() {
        super::super::disable_console_spawning();
        assert!(super::super::console_spawning_disabled());
        assert!(!super::super::attach_stdio_enabled());
        assert!(!enabled());
        #[cfg(windows)]
        let before = unsafe { windows::Win32::System::Console::GetConsoleWindow() };
        emit_console_records();
        #[cfg(windows)]
        assert_eq!(before, unsafe { windows::Win32::System::Console::GetConsoleWindow() });
    }

    fn emit_console_records() {
        use log::Log;
        let raw: log4rs::config::RawConfig = serde_yaml::from_str(r#"
appenders:
  console:
    kind: console
    encoder:
      kind: module_console
root:
  level: debug
  appenders: [console]
"#).unwrap();
        let mut deserializers = log4rs::config::Deserializers::default();
        crate::logging::log4rs::register(&mut deserializers);
        let (appenders, errors) = raw.appenders_lossy(&deserializers);
        assert!(errors.is_empty());
        let config = log4rs::config::Config::builder().appenders(appenders).build(raw.root()).unwrap();
        let logger = log4rs::Logger::new(config);
        for level in [log::Level::Info, log::Level::Debug] {
            logger.log(&log::Record::builder().level(level).module_path(Some("UnchainedPlugin::test"))
                .args(format_args!("hello 世界\nsecond line")).build());
        }
        super::super::print_banner().unwrap();
    }
}
