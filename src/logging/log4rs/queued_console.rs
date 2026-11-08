use std::io;
use std::sync::{atomic::{AtomicUsize, Ordering}, mpsc, Arc, Mutex};

use log::Record;
use log4rs::{append::Append, encode::{Encode, Style, Write}};

const QUEUE_CAPACITY: usize = 128 * 1024;
const PAUSE_THRESHOLD: usize = 100;
const PAUSE_WARNING: &str = "Logging thread pause detected. Be careful selecting text in the console for prolonged periods. It freezes the console log.";
const MAX_RECORD_BYTES: usize = 64 * 1024;
const MAX_STYLE_CHANGES: usize = 256;

#[derive(Debug, Default)]
struct Rendered {
    bytes: Vec<u8>,
    styles: Vec<(usize, Style)>,
    oversized: bool,
}

impl io::Write for Rendered {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_RECORD_BYTES.saturating_sub(self.storage_bytes()) {
            self.oversized = true;
        }
        if !self.oversized { self.bytes.extend_from_slice(bytes); }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl Write for Rendered {
    fn set_style(&mut self, style: &Style) -> io::Result<()> {
        if self.styles.len() == MAX_STYLE_CHANGES
            || self.storage_bytes() + std::mem::size_of::<(usize, Style)>() > MAX_RECORD_BYTES {
            self.oversized = true;
        }
        if !self.oversized { self.styles.push((self.bytes.len(), style.clone())); }
        Ok(())
    }
}

impl Rendered {
    fn storage_bytes(&self) -> usize {
        self.bytes.len() + self.styles.len() * std::mem::size_of::<(usize, Style)>()
    }

    fn replay(&self, writer: &mut dyn Write) -> io::Result<()> {
        let mut start = 0;
        for (offset, style) in &self.styles {
            writer.write_all(&self.bytes[start..*offset])?;
            writer.set_style(style)?;
            start = *offset;
        }
        writer.write_all(&self.bytes[start..])?;
        writer.flush()
    }
}

#[derive(Debug, Default)]
struct Backlog {
    queued: usize,
    warned: bool,
}

impl Backlog {
    fn enqueued(&mut self) -> bool {
        self.queued += 1;
        let warn = self.queued > PAUSE_THRESHOLD && !self.warned;
        self.warned |= warn;
        warn
    }

    fn dequeued(&mut self) {
        self.queued -= 1;
        if self.queued < PAUSE_THRESHOLD { self.warned = false; }
    }
}

/// Only console I/O runs on the worker; formatting retains the caller's metadata.
#[derive(Debug)]
pub(super) struct QueuedConsole {
    encoder: Arc<dyn Encode>,
    sender: mpsc::SyncSender<Rendered>,
    dropped: Arc<AtomicUsize>,
    backlog: Arc<Mutex<Backlog>>,
}

impl QueuedConsole {
    pub(super) fn new(encoder: Box<dyn Encode>, target: log4rs::append::console::Target,
        writer: Option<log4rs::encode::writer::console::ConsoleWriter>)
        -> io::Result<Self> {
        use log4rs::encode::writer::simple::SimpleWriter;
        Self::with_output(encoder, move |record| {
            if let Some(writer) = &writer { return record.replay(&mut writer.lock()); }
            match target {
                log4rs::append::console::Target::Stdout => record.replay(&mut SimpleWriter(io::stdout().lock())),
                log4rs::append::console::Target::Stderr => record.replay(&mut SimpleWriter(io::stderr().lock())),
            }
        }, QUEUE_CAPACITY)
    }

    fn with_output(encoder: Box<dyn Encode>, mut output: impl FnMut(&Rendered) -> io::Result<()> + Send + 'static,
        capacity: usize) -> io::Result<Self> {
        let encoder: Arc<dyn Encode> = encoder.into();
        let worker_encoder = Arc::clone(&encoder);
        let dropped = Arc::new(AtomicUsize::new(0));
        let worker_dropped = Arc::clone(&dropped);
        let backlog = Arc::new(Mutex::new(Backlog::default()));
        let worker_backlog = Arc::clone(&backlog);
        let (sender, receiver) = mpsc::sync_channel::<Rendered>(capacity);
        std::thread::Builder::new().name("unchained-console".into()).spawn(move || {
            while let Ok(record) = receiver.recv() {
                worker_backlog.lock().unwrap_or_else(|error| error.into_inner()).dequeued();
                let count = worker_dropped.swap(0, Ordering::Relaxed);
                let result = (|| {
                    if count != 0 {
                        let mut warning = Rendered::default();
                        let values = [("enclosing_function", "console_worker")];
                        worker_encoder.encode(&mut warning, &Record::builder()
                            .level(log::Level::Warn).target(module_path!()).module_path(Some(module_path!()))
                            .key_values(&values)
                            .args(format_args!("{count} console log records dropped (queue or record limit)"))
                            .build()).map_err(io::Error::other)?;
                        output(&warning)?;
                    }
                    output(&record)
                })();
                if let Err(error) = result {
                    // Error reporting must also stay off the caller's thread.
                    eprintln!("Unchained console output failed: {error}");
                    break;
                }
            }
        })?;
        Ok(Self { encoder, sender, dropped, backlog })
    }

    fn enqueue(&self, mut rendered: Rendered) -> bool {
        if rendered.oversized {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        // Avoid retaining Vec growth slack in the potentially large backlog.
        rendered.bytes.shrink_to_fit();
        rendered.styles.shrink_to_fit();
        let mut backlog = self.backlog.lock().unwrap_or_else(|error| error.into_inner());
        if self.sender.try_send(rendered).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        backlog.enqueued()
    }
}

impl Append for QueuedConsole {
    fn append(&self, record: &Record) -> anyhow::Result<()> {
        let mut rendered = Rendered::default();
        self.encoder.encode(&mut rendered, record)?;
        if self.enqueue(rendered) {
            // Never log while holding the backlog lock: this re-enters the appender.
            crate::swarn!("{PAUSE_WARNING}");
        }
        Ok(())
    }

    // Selection can block console output indefinitely; never wait for it here or on drop.
    fn flush(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use log4rs::encode::{pattern::PatternEncoder, writer::simple::SimpleWriter, Color};
    use std::io::Write as _;
    use std::time::Duration;

    #[test]
    fn pause_warning_rearms_only_below_one_hundred_queued_records() {
        let mut backlog = Backlog::default();
        for _ in 0..100 { assert!(!backlog.enqueued()); }
        assert!(backlog.enqueued());
        assert!(!backlog.enqueued());
        backlog.dequeued();
        backlog.dequeued();
        assert_eq!(backlog.queued, 100);
        assert!(!backlog.enqueued(), "100 must not rearm the warning");
        backlog.dequeued();
        backlog.dequeued();
        assert_eq!(backlog.queued, 99);
        assert!(!backlog.enqueued());
        assert!(backlog.enqueued());
    }

    #[test]
    fn paused_worker_buffers_one_hundred_twenty_eight_k_records() {
        let (entered, started) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let mut first = true;
        let console = QueuedConsole::with_output(Box::new(PatternEncoder::new("{m}")), move |_| {
            if first {
                first = false;
                entered.send(()).unwrap();
                resume.recv().unwrap();
            }
            Ok(())
        }, QUEUE_CAPACITY).unwrap();
        assert!(!console.enqueue(Rendered::default()));
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        for index in 1..=QUEUE_CAPACITY {
            assert_eq!(console.enqueue(Rendered::default()), index == 101);
        }
        assert_eq!(console.dropped.load(Ordering::Relaxed), 0);
        assert!(!console.enqueue(Rendered::default()));
        assert_eq!(console.dropped.load(Ordering::Relaxed), 1);
        assert_eq!(console.backlog.lock().unwrap().queued, QUEUE_CAPACITY);
        release.send(()).unwrap();
    }

    #[test]
    fn record_budget_includes_styles_and_text() {
        let mut packet = Rendered::default();
        packet.set_style(&Style::new()).unwrap();
        let available = MAX_RECORD_BYTES - packet.storage_bytes();
        packet.write_all(&vec![b'x'; available]).unwrap();
        assert!(!packet.oversized);
        assert_eq!(packet.storage_bytes(), MAX_RECORD_BYTES);
        packet.set_style(&Style::new()).unwrap();
        assert!(packet.oversized);
        assert_eq!(packet.storage_bytes(), MAX_RECORD_BYTES);
    }

    #[test]
    fn buffered_output_preserves_utf8_bytes_and_native_style_boundaries() {
        #[derive(Default)]
        struct Capture { bytes: Vec<u8>, styles: Vec<(usize, Style)> }
        impl io::Write for Capture {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.bytes.write(bytes) }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        impl Write for Capture {
            fn set_style(&mut self, style: &Style) -> io::Result<()> {
                self.styles.push((self.bytes.len(), style.clone()));
                Ok(())
            }
        }
        let mut packet = Rendered::default();
        let mut style = Style::new();
        style.text(Color::Yellow);
        packet.write_all("before 世界".as_bytes()).unwrap();
        packet.set_style(&style).unwrap();
        packet.write_all("after".as_bytes()).unwrap();
        packet.set_style(&Style::new()).unwrap();
        let mut output = Capture::default();
        packet.replay(&mut output).unwrap();
        assert_eq!(output.bytes, packet.bytes);
        assert_eq!(output.styles, packet.styles);
    }

    #[test]
    fn stalled_console_does_not_block_file_logging_flush_or_drop() {
        #[derive(Debug)]
        struct FileCapture(Arc<std::sync::Mutex<Vec<String>>>);
        impl Append for FileCapture {
            fn append(&self, record: &Record) -> anyhow::Result<()> {
                self.0.lock().unwrap().push(record.args().to_string());
                Ok(())
            }
            fn flush(&self) {}
        }
        let (entered, started) = mpsc::channel();
        let (release, resume) = mpsc::sync_channel(1);
        let (written, output) = mpsc::channel();
        let mut first = true;
        let console = QueuedConsole::with_output(Box::new(PatternEncoder::new("{m}")), move |record| {
            if first {
                first = false;
                entered.send(()).unwrap();
                resume.recv().unwrap();
            }
            written.send(String::from_utf8(record.bytes.clone()).unwrap()).unwrap();
            Ok(())
        }, 2).unwrap();
        let files = Arc::new(std::sync::Mutex::new(Vec::new()));
        let config = log4rs::Config::builder()
            .appender(log4rs::config::Appender::builder().build("console", Box::new(console)))
            .appender(log4rs::config::Appender::builder().build("file", Box::new(FileCapture(Arc::clone(&files)))))
            .build(log4rs::config::Root::builder().appender("console").appender("file")
                .build(log::LevelFilter::Trace)).unwrap();
        let logger = log4rs::Logger::new(config);
        log::Log::log(&logger, &Record::builder().args(format_args!("first")).build());
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        let (finished, done) = mpsc::channel();
        std::thread::spawn(move || {
            for message in ["second", "third", "dropped"] {
                log::Log::log(&logger, &Record::builder().args(format_args!("{message}")).build());
            }
            log::Log::flush(&logger);
            drop(logger);
            finished.send(()).unwrap();
        });
        let completed = done.recv_timeout(Duration::from_secs(5));
        release.send(()).unwrap();
        completed.unwrap();
        assert_eq!(*files.lock().unwrap(), ["first", "second", "third", "dropped"]);
        assert_eq!(output.recv_timeout(Duration::from_secs(5)).unwrap(), "first");
        assert!(output.recv_timeout(Duration::from_secs(5)).unwrap().starts_with("1 console log records dropped"));
        assert_eq!(output.recv_timeout(Duration::from_secs(5)).unwrap(), "second");
        assert_eq!(output.recv_timeout(Duration::from_secs(5)).unwrap(), "third");
    }

    #[test]
    #[ignore = "manual caller-side console queue benchmark without terminal I/O"]
    fn caller_overhead_benchmark() {
        use crate::logging::log4rs::palette::PaletteEncoderConfig;
        const ITERATIONS: usize = 20_000;
        let encoder = || -> Box<dyn Encode> {
            if std::env::var_os("UNCHAINED_BENCH_WITHOUT_TIMESTAMP").is_some() {
                return Box::new(crate::logging::log4rs::pattern::PatternEncoder::new(
                    "[{l:>5}] [{M}::{fn}] {m}{n}"));
            }
            let config: PaletteEncoderConfig = serde_yaml::from_value(
                crate::logging::test_resources::encoder("defaults")).unwrap();
            Box::new(config.build().unwrap())
        };
        for round in 0..5 {
            let direct = encoder();
            let queued = QueuedConsole::with_output(encoder(), |packet| {
                packet.replay(&mut SimpleWriter(io::sink()))
            }, QUEUE_CAPACITY).unwrap();
            for queue in [round % 2 == 0, round % 2 != 0] {
                let values = [("enclosing_function", "caller_overhead_benchmark")];
                let start = std::time::Instant::now();
                for index in 0..ITERATIONS {
                    match &Record::builder().level(log::Level::Info)
                        .module_path(Some("UnchainedPlugin::resolvers::gameplay"))
                        .key_values(&values)
                        .args(format_args!("event {}: {} -> {}", std::hint::black_box(index), "PlayerA", "PlayerB"))
                        .build() {
                        record => {
                            if queue { queued.append(record).unwrap(); }
                            else { direct.encode(&mut SimpleWriter(io::sink()), record).unwrap(); }
                        }
                    }
                }
                eprintln!("round={round} queued={queue} caller_ns_per_record={} drops={}",
                    start.elapsed().as_nanos() / ITERATIONS as u128, queued.dropped.load(Ordering::Relaxed));
            }
        }
    }

    #[test]
    fn oversized_console_records_are_dropped_whole() {
        let (written, output) = mpsc::channel();
        let console = QueuedConsole::with_output(Box::new(PatternEncoder::new("{m}")), move |record| {
            let mut bytes = Vec::new();
            record.replay(&mut SimpleWriter(&mut bytes))?;
            written.send(String::from_utf8(bytes).unwrap()).unwrap();
            Ok(())
        }, 2).unwrap();
        let large = "界".repeat(MAX_RECORD_BYTES);
        console.append(&Record::builder().args(format_args!("{large}")).build()).unwrap();
        console.append(&Record::builder().args(format_args!("small")).build()).unwrap();
        assert!(output.recv_timeout(Duration::from_secs(5)).unwrap().starts_with("1 console log records dropped"));
        assert_eq!(output.recv_timeout(Duration::from_secs(5)).unwrap(), "small");
        let mut packet = Rendered::default();
        for _ in 0..=MAX_STYLE_CHANGES { packet.set_style(&Style::new()).unwrap(); }
        assert!(packet.oversized);
        assert_eq!(packet.styles.len(), MAX_STYLE_CHANGES);
    }
}
