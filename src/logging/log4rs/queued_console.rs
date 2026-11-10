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
    use log4rs::encode::Color;
    use std::io::Write as _;

    #[test]
    fn full_queue_counts_drop_without_growing_backlog() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        let console = QueuedConsole {
            encoder: Arc::new(log4rs::encode::pattern::PatternEncoder::new("{m}")),
            sender,
            dropped: Arc::new(AtomicUsize::new(0)),
            backlog: Arc::new(Mutex::new(Backlog::default())),
        };
        console.enqueue(Rendered::default());
        console.enqueue(Rendered::default());
        assert_eq!(console.dropped.load(Ordering::Relaxed), 1);
        assert_eq!(console.backlog.lock().unwrap().queued, 1);
    }

    #[test]
    fn pause_warning_fires_once_above_threshold() {
        let mut backlog = Backlog { queued: PAUSE_THRESHOLD - 1, warned: false };
        assert!(!backlog.enqueued());
        assert!(backlog.enqueued());
        assert!(!backlog.enqueued());
    }

    #[test]
    fn pause_warning_rearms_below_threshold() {
        let mut backlog = Backlog { queued: PAUSE_THRESHOLD + 1, warned: true };
        backlog.dequeued();
        assert!(backlog.warned);
        backlog.dequeued();
        assert!(!backlog.warned);
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
        let mut packet = Rendered::default();
        let mut style = Style::new();
        style.text(Color::Yellow);
        packet.write_all("before 世界".as_bytes()).unwrap();
        packet.set_style(&style).unwrap();
        packet.write_all("after".as_bytes()).unwrap();
        packet.set_style(&Style::new()).unwrap();
        let mut output = Rendered::default();
        packet.replay(&mut output).unwrap();
        assert_eq!(output.bytes, packet.bytes);
        assert_eq!(output.styles, packet.styles);
    }

    #[test]
    fn oversized_text_is_not_buffered() {
        let mut packet = Rendered::default();
        packet.write_all(&vec![b'x'; MAX_RECORD_BYTES + 1]).unwrap();
        assert!(packet.oversized);
        assert!(packet.bytes.is_empty());
    }

    #[test]
    fn style_count_is_bounded() {
        let mut packet = Rendered::default();
        for _ in 0..=MAX_STYLE_CHANGES { packet.set_style(&Style::new()).unwrap(); }
        assert!(packet.oversized);
        assert_eq!(packet.styles.len(), MAX_STYLE_CHANGES);
    }
}
