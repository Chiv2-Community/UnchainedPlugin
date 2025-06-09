// use anyhow::Result;
// use chrono::Local;
// use std::net::UdpSocket;
// use std::sync::mpsc::{self, Sender};
// use std::thread;
// use std::time::Duration;

// fn format_syslog_message(hostname: &str, tag: &str, message: &str) -> String {
//     let stamp = Local::now().format("%b %d %H:%M:%S");
//     let pri = 13;
//     format!("<{pri}>{stamp} {hostname} {tag}: {message}")
// }

// pub fn spawn_syslog_sender(syslog_addr: &'static str) -> Sender<String> {
//     let (tx, rx) = mpsc::channel::<String>();
//     thread::spawn(move || {
//         let socket = UdpSocket::bind("0.0.0.0:0")
//             .expect("Failed to bind udp socket");

//         for line in rx {
//             let msg = format_syslog_message("rust-client", "rustapp", &line);
//             if let Err(e) = socket.send_to(msg.as_bytes(), syslog_addr) {
//                 eprintln!("Failed to send syslog msg: {e}");
//             }
//         }
//     });
//     tx
// }

// pub fn start_log_generator(tx: Sender<String>) {
//     thread::spawn(move || {
//         let messages = [
//             "Some test",
//             "Blabliblub",
//         ];
//         for msg in messages.iter().cycle() {
//             if tx.send(msg.to_string()).is_err() { // recv fail
//                 break;
//             }
//             thread::sleep(Duration::from_secs(2));
//         }
//     });
// }

use std::cell::RefCell;
use std::io::{self, Cursor, Write};
use std::sync::{Arc, Mutex};
use log4rs::append::file::FileAppender;
use log4rs::filter::threshold::ThresholdFilter;
use once_cell::sync::Lazy;

const DEFAULT_BUF_SIZE: usize = 4096;
type PersistentBuf = Cursor<Vec<u8>>;

thread_local! {
    static PERSISTENT_BUF: RefCell<PersistentBuf> =
        RefCell::new(Cursor::new(Vec::with_capacity(DEFAULT_BUF_SIZE)));
}

// Optional: for some kind of global shared output string
pub static SYSLOG_BUF: Lazy<Arc<Mutex<Option<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub struct BufWriter;

impl BufWriter {
    pub fn new() -> Self {
        PERSISTENT_BUF.with(|buf| {
            buf.borrow_mut().set_position(0); // Reset position
        });
        BufWriter
    }

    /// Returns the current buffer as a UTF-8 string and resets buffer.
    pub fn flush_to_string(&self) -> String {
        PERSISTENT_BUF.with(|buf| {
            let mut buf = buf.borrow_mut();
            let pos = buf.position() as usize;
            let slice = &buf.get_ref()[..pos];
            match std::str::from_utf8(slice) {
                Ok(s) => s.to_string(),
                Err(_) => "<invalid utf8>".to_string(),
            }
        })
    }
}

impl Write for BufWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        PERSISTENT_BUF.with(|pbuf| pbuf.borrow_mut().write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        // In case you want to trigger network send from here
        let log_line = self.flush_to_string();
        // println!("Sending syslog: {}", log_line);
        // send_to_syslog(log_line); // You can call your network send function here
        Ok(())
    }
}
impl log4rs::encode::Write for BufWriter {}

// https://en.wikipedia.org/wiki/Syslog

use chrono::Local;
use std::{fmt};
use std::net::UdpSocket;

use log::{info, Level, Record};
use log4rs::{
    append::{console::{ConsoleAppender, Target}, Append},
    config::{Appender, Config, Root},
    encode::{pattern::PatternEncoder, Encode},
    init_config,
    // ConfigBuilder,
};

// #[derive(Debug)]
pub struct SyslogAppender {
    // writer: Writer,
    encoder: Box<dyn Encode>,
    do_write: bool,
    socket: Arc<UdpSocket>,
    target_addr: String,
    hostname: String,
    tag: String,
}

impl Append for SyslogAppender {
    fn append(&self, record: &Record) -> anyhow::Result<()> {
        // TODO: pre-process record (long paths etc)
        let mut buf = BufWriter::new();
        self.encoder.encode(&mut buf, record)?;
        // println!("SYSLOG {}", buf.flush_to_string());
        // let msg = format!("{}", record.args());
        // let full_msg = self.format_syslog_message(record.level(), &msg);
        // println!("{:#?}", record);
        let _ = self.socket.send_to(buf.flush_to_string().as_bytes(), &self.target_addr);
        // let _ = self.socket.send_to(buf.as_bytes(), &self.target_addr);
        Ok(())
    }

    fn flush(&self) {}
}

impl SyslogAppender {
    /// Create new builder for `SyslogAppender`.
    pub fn builder() -> SyslogAppenderBuilder {
        SyslogAppenderBuilder {
            encoder: None,
            target: Target::Stdout,
        }
    }
}
// TODO: syslog hostname (cli?)
// impl SyslogAppender {
//     /// Creates a new `SyslogAppender` builder.
//     pub fn builder() -> SyslogAppenderBuilder {
//         SyslogAppenderBuilder {
//             encoder: None,
//             target: Target::Stdout,
//             tty_only: false,
//         }
//     }
// }
/// A builder for `SyslogAppender`s.
pub struct SyslogAppenderBuilder {
    encoder: Option<Box<dyn Encode>>,
    target: Target,
}

impl SyslogAppenderBuilder {
    /// Sets the output encoder for the `SyslogAppender`.
    pub fn encoder(mut self, encoder: Box<dyn Encode>) -> SyslogAppenderBuilder {
        self.encoder = Some(encoder);
        self
    }

    /// Consumes the `SyslogAppenderBuilder`, producing a `SyslogAppender`.
    pub fn build(self) -> SyslogAppender {
        // let writer = match self.target {
        //     Target::Stderr => match SyslogWriter::stderr() {
        //         Some(writer) => Writer::Tty(writer),
        //         None => Writer::Raw(StdWriter::stderr()),
        //     },
        //     Target::Stdout => match SyslogWriter::stdout() {
        //         Some(writer) => Writer::Tty(writer),
        //         None => Writer::Raw(StdWriter::stdout()),
        //     },
        // };

        // let do_write = writer.is_tty() || !self.tty_only;

        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        SyslogAppender {
            // writer,
            encoder: self
                .encoder
                .unwrap_or_else(|| Box::<PatternEncoder>::default()),
            do_write: true,
            socket: Arc::new(socket),
            target_addr: "127.0.0.1:514".to_string(),
            hostname: "unchained".to_string(),
            tag: "some_tag".to_string(),
        }
    }
}

impl SyslogAppender {
    pub fn new(syslog_addr: &str, hostname: &str, tag: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        Ok(Self {
            socket: Arc::new(socket),
            target_addr: syslog_addr.to_string(),
            hostname: hostname.to_string(),
            tag: tag.to_string(),
            encoder: todo!(),
            do_write: todo!(),
        })
    }

    fn format_syslog_message(&self, level: Level, msg: &str) -> String {
        let pri = match level {
            Level::Error => 3,
            Level::Warn => 4,
            Level::Info => 6,
            Level::Debug | Level::Trace => 7,
        } + 8; // facility 1 (user)
        // TODO: Extend facility, let user provide it. Enum?
        //      Also syslog it supports more severity levels

        let timestamp = Local::now().format("%b %d %H:%M:%S");
        format!(
            "<{}>{} {} {}: {}",
            pri, timestamp, self.hostname, self.tag, msg
        )
    }
}
impl fmt::Debug for SyslogAppender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyslogAppender")
            .field("target_addr", &self.target_addr)
            .field("hostname", &self.hostname)
            .field("tag", &self.tag)
            .finish()
    }
}

pub fn init_syslog() -> anyhow::Result<()> {
    // let console = ConsoleAppender::builder()
    //     .encoder(Box::new(PatternEncoder::new(
    //         "[{d(%Y-%m-%d %H:%M:%S)}] [{l:5}] {m}{n}",
    //     )))
    //     .build();
    info!(target:"syslog_init", "hello");
    info!(target:"syslog", "hello syslog");
    
    // use function name
    let console = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            // "[{d(%Y-%m-%d %H:%M:%S)}] {h:1}[{l:5}]{h:-1} {m}{n}",
        // "[{d(%Y-%m-%d %H:%M:%S)}] {h:1}[{l:5}]{h:-1} {m}{n}",  
            
            // TODO: make this configurable opt? Maybe only for syslog or only file log
            // "[{d(%Y-%m-%d %H:%M:%S)} {h({l:5})}] [{f}:{L}] {m}{n}", // Log file name and line
            "{h([{d(%H:%M:%S)} {l:5}])} {m}{n}",
        )))
        .build();

        let syslog = SyslogAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            // "[{d(%Y-%m-%d %H:%M:%S)}] {h:1}[{l:5}]{h:-1} {m}{n}",
        // "[{d(%Y-%m-%d %H:%M:%S)}] {h:1}[{l:5}]{h:-1} {m}{n}",  
            
            // TODO: make this configurable opt? Maybe only for syslog or only file log
            "[{d(%Y-%m-%d %H:%M:%S)}] [{l:5}] {m}{n}", // Log file name and line
        )))
        .build();
        let file = FileAppender::builder()
        // .encoder(Box::new(PatternEncoder::new("[{d(%Y-%m-%d %H:%M:%S)}] [{l:5}] [{M}] [{f}:{L}] {m}{n}\n")))
        .encoder(Box::new(PatternEncoder::new("[{d(%Y-%m-%d %H:%M:%S)}] {P} [{l:6}] [{t}] {m}{n}")))
        // .build("my_log_file.log")?;
        .build(r"U:\Unchained\UnchainedSleuth\unchained.log")?;
    // Set up our custom syslog UDP appender
    // let syslog = SyslogAppender::new(
    //     "127.0.0.1:514", // TODO: config option
    //     "rustlib", // Build from launch type?
    //     "unchained",
    // )?;
    let console_filter = ThresholdFilter::new(log::LevelFilter::Info);

    // Build the config programmatically
    let config = Config::builder()
        .appender(Appender::builder().filter(Box::new(console_filter)).build("console", Box::new(console)))
        .appender(Appender::builder().build("syslog", Box::new(syslog)))
        .appender(Appender::builder().build("file", Box::new(file)))
        .build(
            Root::builder()
                .appender("console")
                .appender("syslog")
                .appender("file")
                .build(log::LevelFilter::Debug),
        )?;

    init_config(config)?;

    Ok(())
}