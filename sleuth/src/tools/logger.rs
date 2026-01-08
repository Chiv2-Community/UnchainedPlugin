

use log4rs::{Config, append::{console::ConsoleAppender, file::FileAppender}, config::{Appender, Logger, Root}, encode::pattern::PatternEncoder, filter::threshold::ThresholdFilter, init_config};

#[cfg(feature="syslog-client")]
use super::syslog::SyslogAppender;
use std::backtrace::Backtrace;
use std::panic;
use log::{LevelFilter, error};

pub fn setup_panic_logger() {
    panic::set_hook(Box::new(|info| {
        let backtrace = Backtrace::force_capture();
        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<dyn Any>",
            },
        };

        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        error!(
            "PANIC at {}: {}\nStack Backtrace:\n{}",
            location, msg, backtrace
        );
    }));
}

pub fn init_syslog() -> anyhow::Result<()> {
    // use function name
    let console = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            // TODO: make this configurable opt? Maybe only for syslog or only file log
            // "[{d(%Y-%m-%d %H:%M:%S)} {h({l:5})}] [{f}:{L}] {m}{n}", // Log file name and line
            "{h([{d(%H:%M:%S)} {l:5}])} {m}{n}",
        )))
        .build();

        #[cfg(feature="syslog-client")]
        let syslog = SyslogAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            // TODO: make this configurable opt? Maybe only for syslog or only file log
            // FIXME: Nihi: add valid syslog pattern in Appender
            "[{d(%Y-%m-%d %H:%M:%S)}] [{l:5}] {m}{n}", // Log file name and line
        )))
        .build();

        let file = FileAppender::builder()
        // .encoder(Box::new(PatternEncoder::new("[{d(%Y-%m-%d %H:%M:%S)}] [{l:5}] [{M}] [{f}:{L}] {m}{n}\n")))
        // .encoder(Box::new(PatternEncoder::new("[{d(%Y-%m-%d %H:%M:%S)}] {P} [{l:6}] [{t}] {m}{n}")))
        .encoder(Box::new(PatternEncoder::new("[{d(%Y-%m-%d %H:%M:%S)} {P} {l:6}| {t:10}] {m}{n}")))
        // .build("my_log_file.log")?;
        // .build(r"U:\Unchained\UnchainedSleuth\unchained.log")?; // FIXME: Nihi: LOCAL FILE
    .build(r"unchained.log")?; // FIXME: Nihi: LOCAL FILE
        let kismet = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("[{d(%Y-%m-%d %H:%M:%S)} {P} {l:6}| {t} ] {m}{n}")))
        // .build(r"U:\Unchained\UnchainedSleuth\kismet.log")?;
    .build(r"kismet.log")?;
    let console_filter = ThresholdFilter::new(log::LevelFilter::Info);
    // let console_filter: MetaDataFilter = MetaDataFilter::new(log::LevelFilter::Info);

    // Build the config programmatically
    let mut builder = Config::builder()
    .appender(Appender::builder()
    .filter(Box::new(console_filter))
    .build("console", Box::new(console)));

    builder = builder
        .logger(Logger::builder().build("serenity", log::LevelFilter::Warn))
        .logger(Logger::builder().build("tracing", log::LevelFilter::Warn));

    // #[cfg(feature="syslog-client")]
    // {
    //     builder = builder
    //     .appender(Appender::builder().build("syslog", Box::new(syslog)));
    // }

    // let config = Config::builder()
    //     .appender(Appender::builder().filter(Box::new(console_filter)).build("console", Box::new(console)))
    //     // .appender(Appender::builder().build("syslog", Box::new(syslog)))
    let config = builder
        .appender(Appender::builder().build("file", Box::new(file)))
        .appender(Appender::builder().build("kismet", Box::new(kismet)))
        .appender(Appender::builder().build("syslog", Box::new(syslog)))
        .build(
            Root::builder()
                .appender("console")
                .appender("syslog")
                .appender("file")
                // .additive(false)
                // .appender("kismet")
                .build(log::LevelFilter::Debug),
        )?;

    init_config(config)?;
    setup_panic_logger();

    Ok(())
}
