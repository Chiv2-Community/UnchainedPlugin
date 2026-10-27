//! Opt-in console transport. Game threads only encode and try a bounded queue.
use std::{io::{self, Write}, net::{SocketAddr, TcpStream},
    sync::{atomic::{AtomicU64, Ordering}, mpsc::{self, SyncSender, TrySendError}, OnceLock}, time::Duration};

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

fn settings(endpoint: &str, token: &str) -> Option<(SocketAddr, String)> {
    let address: SocketAddr = endpoint.parse().ok()?;
    if !address.ip().is_loopback() || address.port() == 0 || token.len() != 64
        || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) { return None; }
    Some((address, token.to_owned()))
}

fn forwarder() -> Option<&'static Forwarder> {
    FORWARDER.get_or_init(|| {
        let (address, token) = settings(&std::env::var("UNCHAINED_CONSOLE_ENDPOINT").ok()?,
            &std::env::var("UNCHAINED_CONSOLE_TOKEN").ok()?)?;
        let (sender, receiver) = mpsc::sync_channel(CAPACITY);
        std::thread::Builder::new().name("console-forward".into()).spawn(move || {
            // A failed or slow receiver cannot stall game threads. File logging remains active.
            let _ = (|| -> io::Result<()> {
                let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
                stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                serde_json::to_writer(&mut stream, &serde_json::json!({
                    "version": 1, "kind": "hello", "token": token
                }))?;
                stream.write_all(b"\n")?;
                // Wait for the receiver to accept the token and protocol before sending output.
                let mut ack = [0; 3];
                std::io::Read::read_exact(&mut stream, &mut ack)?;
                if ack != *b"OK\n" { return Err(io::Error::other("console handshake rejected")); }
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

pub(crate) fn enabled() -> bool { !super::attach_std_enabled() && forwarder().is_some() }

pub(crate) fn send(kind: &'static str, level: Option<log::Level>, text: String) {
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

    #[test]
    fn endpoint_is_loopback_and_token_is_fixed_length_hex() {
        let token = "a".repeat(64);
        assert!(settings("127.0.0.1:1234", &token).is_some());
        assert!(settings("[::1]:1234", &token).is_some());
        for endpoint in ["0.0.0.0:1234", "192.0.2.1:1234", "localhost:1234", "127.0.0.1:0"] {
            assert!(settings(endpoint, &token).is_none());
        }
        assert!(settings("127.0.0.1:1234", "secret\n").is_none());
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

    #[test]
    fn actual_appender_and_banner_use_authenticated_transport() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let marker = std::env::temp_dir().join(format!("forward-test-{}-{unique}", std::process::id()));
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", &format!("{}::forward_child", module_path!().split_once("::").unwrap().1)])
            .env("UNCHAINED_CONSOLE_ENDPOINT", listener.local_addr().unwrap().to_string())
            .env("UNCHAINED_CONSOLE_TOKEN", "b".repeat(64))
            .env("UNCHAINED_FORWARD_TEST_MARKER", &marker)
            .stdout(std::process::Stdio::null()).spawn().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let (stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(std::time::Instant::now() < deadline, "forwarder did not connect");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("{error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let hello: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(hello["token"], "b".repeat(64));
        assert_eq!(hello["version"], 1);
        reader.get_mut().write_all(b"OK\n").unwrap();
        let mut frames = Vec::new();
        for _ in 0..3 {
            line.clear();
            reader.read_line(&mut line).unwrap();
            assert!(line.len() < 128 * 1024);
            frames.push(serde_json::from_str::<serde_json::Value>(&line).unwrap());
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
    #[ignore = "subprocess fixture for actual_appender_and_banner_use_authenticated_transport"]
    fn forward_child() {
        emit_console_records();
        let marker = std::env::var("UNCHAINED_FORWARD_TEST_MARKER").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !std::path::Path::new(&marker).exists() {
            assert!(std::time::Instant::now() < deadline, "receiver did not acknowledge test frames");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn attach_std_preserves_pipes_and_overrides_ipc_without_allocating_a_console() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args(["--ignored", "--exact", "--nocapture", &format!("{}::attach_std_child", module_path!().split_once("::").unwrap().1)])
            .env("UNCHAINED_CONSOLE_ENDPOINT", listener.local_addr().unwrap().to_string())
            .env("UNCHAINED_CONSOLE_TOKEN", "b".repeat(64));
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
        assert!(matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock));
    }

    #[test]
    #[ignore = "subprocess fixture for attach_std_preserves_pipes_and_overrides_ipc_without_allocating_a_console"]
    fn attach_std_child() {
        #[cfg(windows)]
        let before = unsafe { windows::Win32::System::Console::GetConsoleWindow() };
        super::super::attach_std();
        assert!(!enabled());
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
