//! Opt-in console transport. Game threads only encode and try a bounded queue.
use std::{io::{self, Write},
    sync::{atomic::{AtomicU64, Ordering}, mpsc::{self, SyncSender, TrySendError}, OnceLock}};

use serde::Serialize;
use log4rs::encode::{Encode, json::JsonEncoder, writer::simple::SimpleWriter};

const CAPACITY: usize = 256;
const MAX_TEXT: usize = 16 * 1024;
const MAX_RECORD: usize = 120 * 1024;
static FORWARDER: OnceLock<Option<Forwarder>> = OnceLock::new();

struct Forwarder {
    sender: SyncSender<Event>,
    dropped: AtomicU64,
}

#[derive(Serialize)]
struct Event {
    version: u8,
    kind: &'static str,
    record: Box<serde_json::value::RawValue>,
    dropped: u64,
}

impl Event {
    #[cfg(test)]
    fn json(&self) -> serde_json::Value { serde_json::from_str(self.record.get()).unwrap() }

    fn new(kind: &'static str, level: Option<log::Level>, text: String) -> anyhow::Result<Self> {
        let mut event = Self::from_record(&log::Record::builder()
            .level(level.unwrap_or(log::Level::Info)).args(format_args!("{text}")).build())?;
        event.kind = kind;
        Ok(event)
    }

    fn from_record(record: &log::Record<'_>) -> anyhow::Result<Self> {
        let mut buffer = TextBuffer::default();
        std::fmt::write(&mut buffer, *record.args())
            .map_err(|_| io::Error::other("Log message formatting failed"))?;
        let text = buffer.finish();
        let mut encoded = RecordBuffer(Vec::new());
        JsonEncoder::new().encode(&mut SimpleWriter(&mut encoded),
            &record.to_builder().args(format_args!("{text}")).build())?;
        Ok(Self { version: 1, kind: "log", record: serde_json::from_slice(&encoded.0)?, dropped: 0 })
    }
}

struct RecordBuffer(Vec<u8>);
impl Write for RecordBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_RECORD.saturating_sub(self.0.len()) {
            return Err(io::Error::other("Structured log record exceeds limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
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
        let _ = forwarder.send_encoded(Event::new(kind, level, text));
    }
}

pub(crate) fn send_record(record: &log::Record<'_>) -> anyhow::Result<()> {
    if super::attach_stdio_enabled() { return Ok(()); }
    if let Some(forwarder) = forwarder() {
        forwarder.send_encoded(Event::from_record(record))?;
    }
    Ok(())
}

impl Forwarder {
    fn send_encoded(&self, event: anyhow::Result<Event>) -> anyhow::Result<()> {
        match event {
            Ok(event) => self.send(event),
            Err(error) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                return Err(error);
            }
        }
        Ok(())
    }

    fn send(&self, mut event: Event) {
        event.dropped = self.dropped.swap(0, Ordering::Relaxed);
        if let Err(TrySendError::Full(event) | TrySendError::Disconnected(event)) = self.sender.try_send(event) {
            self.dropped.fetch_add(event.dropped.saturating_add(1), Ordering::Relaxed);
        }
    }
}

// Bound encoding too: one pathological log message must not allocate an unbounded frame.
#[derive(Default)]
struct TextBuffer {
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

impl std::fmt::Write for TextBuffer {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.write_all(text.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

impl TextBuffer {
    fn finish(self) -> String {
        let mut text = String::from_utf8(self.bytes)
            .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
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
        let event = || Event::new("log", None, "message".into()).unwrap();
        forwarder.send(event());
        forwarder.send(event());
        forwarder.send(event());
        assert_eq!(receiver.recv().unwrap().dropped, 0);
        forwarder.send(event());
        assert_eq!(receiver.recv().unwrap().dropped, 2);
        assert!(forwarder.send_encoded(Err(anyhow::anyhow!("encoding failed"))).is_err());
        forwarder.send(event());
        assert_eq!(receiver.recv().unwrap().dropped, 1);
    }

    #[test]
    fn valid_text_reuses_owned_buffer() {
        for input in ["ordinary log record", "hello 世界 😀"] {
            let mut buffer = TextBuffer::default();
            buffer.write_all(input.as_bytes()).unwrap();
            let pointer = buffer.bytes.as_ptr();
            let text = buffer.finish();
            assert_eq!(text, input);
            assert_eq!(text.as_ptr(), pointer);
        }
    }

    #[test]
    fn invalid_text_is_repaired() {
        for (input, expected) in [
            (&b"a\xffb"[..], "a�b"),
            (&b"\xe2\x82"[..], "�"),
            (&b"\xf0\x28\x8c\x28"[..], "�(�("),
        ] {
            let mut buffer = TextBuffer::default();
            buffer.write_all(input).unwrap();
            assert_eq!(buffer.finish(), expected);
        }
    }

    #[test]
    fn split_codepoints_and_multibyte_truncation_preserve_text_policy() {
        let mut buffer = TextBuffer::default();
        for byte in "世界😀".as_bytes() { buffer.write_all(&[*byte]).unwrap(); }
        assert_eq!(buffer.finish(), "世界😀");

        let mut buffer = TextBuffer::default();
        buffer.write_all("界".repeat(MAX_TEXT).as_bytes()).unwrap();
        assert_eq!(buffer.finish(), format!("{} [truncated]", "界".repeat((MAX_TEXT - " [truncated]".len()) / 3)));
    }

    #[test]
    fn log4rs_records_preserve_json_across_worker_handoff() {
        for level in [Some(log::Level::Error), Some(log::Level::Warn), Some(log::Level::Info),
            Some(log::Level::Debug), Some(log::Level::Trace), None] {
            let kind = if level.is_some() { "log" } else { "banner" };
            let event = Event::new(kind, level, "hello 世界".into()).unwrap();
            let expected = serde_json::json!({ "version": 1, "kind": kind,
                "record": event.record, "dropped": 0 });
            assert_eq!(expected["record"]["level"], level.unwrap_or(log::Level::Info).as_str());
            assert_eq!(expected["record"]["message"], "hello 世界");
            assert!(expected["record"]["thread_id"].as_u64().unwrap() > 0);
            let encoded = std::thread::spawn(move || serde_json::to_value(&event).unwrap()).join().unwrap();
            assert_eq!(encoded, expected);
        }
    }

    #[test]
    fn structured_records_separate_message_and_callsite_metadata() {
        use log::Log;
        #[derive(Default)]
        struct Capture(std::sync::Mutex<Vec<Event>>);
        impl Log for Capture {
            fn enabled(&self, _: &log::Metadata<'_>) -> bool { true }
            fn log(&self, record: &log::Record<'_>) {
                self.0.lock().unwrap().push(Event::from_record(record).unwrap());
            }
            fn flush(&self) {}
        }
        log::set_max_level(log::LevelFilter::Trace);
        let logger = Capture::default();
        let expected_line = line!() + 1;
        sinfo!(logger: &logger, fn; "hello {}\nsecond line", "世界");
        sinfo!(logger: &logger, "[literal prefix] stays");
        sinfo!(logger: &logger, fn; "{}", "x".repeat(MAX_TEXT));
        let events = logger.0.lock().unwrap();
        let event = &events[0];
        assert_eq!(event.version, 1);
        let record = event.json();
        assert_eq!(record["message"], "[structured_records_separate_message_and_callsite_metadata] hello 世界\nsecond line");
        assert_eq!(record["module_path"], module_path!());
        assert_eq!(record["attributes"]["__unchained_function"], "structured_records_separate_message_and_callsite_metadata");
        assert_eq!(record["line"], expected_line);
        assert!(record["thread_id"].as_u64().unwrap() > 0);
        assert!(record["time"].as_str().unwrap().contains('T'));
        assert_eq!(events[1].json()["message"], "[literal prefix] stays");
        let record = events[2].json();
        let text = record["message"].as_str().unwrap();
        assert_eq!(text.len(), MAX_TEXT);
        assert!(text.ends_with(" [truncated]"));
    }

    #[test]
    fn unavailable_source_metadata_is_omitted_and_message_is_not_parsed() {
        let event = Event::from_record(&log::Record::builder().level(log::Level::Warn)
            .args(format_args!("[INFO] [module] literal content")).build()).unwrap();
        let record = event.json();
        assert!(record.get("module_path").is_none());
        assert!(record.get("line").is_none());
        assert_eq!(record["message"], "[INFO] [module] literal content");
    }

    #[test]
    fn log4rs_captures_attributes_targets_and_thread_before_handoff() {
        let event = std::thread::Builder::new().name("emitting-thread".into()).spawn(|| {
            let values = [("request", 42u64)];
            let event = Event::from_record(&log::Record::builder()
                .target("custom-target").module_path(Some("source-module"))
                .file(Some("source.rs")).line(Some(123)).key_values(&values)
                .args(format_args!("message")).build()).unwrap();
            #[cfg(windows)]
            assert_eq!(event.json()["thread_id"], unsafe {
                windows::Win32::System::Threading::GetCurrentThreadId()
            });
            event
        }).unwrap().join().unwrap();
        let record = event.json();
        assert_eq!(record["thread"], "emitting-thread");
        assert_eq!(record["target"], "custom-target");
        assert_eq!(record["module_path"], "source-module");
        assert_eq!(record["file"], "source.rs");
        assert_eq!(record["line"], 123);
        assert_eq!(record["attributes"]["request"], "42");
        assert!(record["mdc"].is_object());
    }

    #[test]
    fn metadata_and_escaped_frames_remain_bounded() {
        let long = "界".repeat(1024);
        let values = [("__unchained_function", long.as_str())];
        let event = Event::from_record(&log::Record::builder().module_path(Some(&long))
            .key_values(&values).args(format_args!("{}", "\0".repeat(MAX_TEXT + 1))).build()).unwrap();
        let record = event.json();
        assert_eq!(record["module_path"], long);
        assert_eq!(record["attributes"]["__unchained_function"], long);
        assert!(record["message"].as_str().unwrap().ends_with(" [truncated]"));
        assert!(serde_json::to_vec(&event).unwrap().len() < 128 * 1024);
        let oversized = "x".repeat(MAX_RECORD);
        assert!(Event::from_record(&log::Record::builder().module_path(Some(&oversized)).build()).is_err());
    }

    #[test]
    fn formatting_errors_do_not_emit_partial_messages() {
        struct FailedDisplay;
        impl std::fmt::Display for FailedDisplay {
            fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Err(std::fmt::Error) }
        }
        assert!(Event::from_record(&log::Record::builder()
            .args(format_args!("partial {}", FailedDisplay)).build()).is_err());
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
        assert_eq!(frames[0]["record"]["level"], "INFO");
        let text = frames[0]["record"]["message"].as_str().unwrap();
        assert_eq!(frames[0]["version"], 1);
        assert_eq!(text, "hello 世界\nsecond line");
        assert_eq!(frames[0]["record"]["module_path"], "UnchainedPlugin::test");
        assert!(frames[0]["record"]["thread_id"].as_u64().unwrap() > 0);
        assert!(!text.contains('\x1b'));
        assert_eq!(frames[1]["record"]["level"], "DEBUG");
        assert_eq!(frames[2]["kind"], "banner");
        assert!(!frames[2]["record"]["message"].as_str().unwrap().contains('\x1b'));
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
