

use log4rs::{append::{console::ConsoleAppender, file::FileAppender}, config::{Appender, Root}, encode::pattern::PatternEncoder, filter::threshold::ThresholdFilter, init_config, Config};

use super::syslog::SyslogAppender;

use log::LevelFilter;
use log4rs::config::{ Logger};

fn main() {
    let stdout = ConsoleAppender::builder().build();

    let requests = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build("log/requests.log")
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("requests", Box::new(requests)))
        .logger(Logger::builder().build("app::backend::db", LevelFilter::Info))
        .logger(Logger::builder()
            .appender("requests")
            .additive(false)
            .build("app::requests", LevelFilter::Info))
        .build(Root::builder().appender("stdout").build(LevelFilter::Warn))
        .unwrap();

    let handle = log4rs::init_config(config).unwrap();

    // use handle to change logger configuration at runtime
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
        .build(r"U:\Unchained\UnchainedSleuth\unchained.log")?; // FIXME: Nihi: LOCAL FILE
        let kismet = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("[{d(%Y-%m-%d %H:%M:%S)} {P} {l:6}| {t} ] {m}{n}")))
        .build(r"U:\Unchained\UnchainedSleuth\kismet.log")?;
    let console_filter = ThresholdFilter::new(log::LevelFilter::Info);
    // let console_filter: MetaDataFilter = MetaDataFilter::new(log::LevelFilter::Info);

    // Build the config programmatically
    let config = Config::builder()
        .appender(Appender::builder().filter(Box::new(console_filter)).build("console", Box::new(console)))
        // .appender(Appender::builder().build("syslog", Box::new(syslog)))
        .appender(Appender::builder().build("file", Box::new(file)))
        .appender(Appender::builder().build("kismet", Box::new(kismet)))
        .build(
            Root::builder()
                .appender("console")
                // .appender("syslog")
                .appender("file")
                // .additive(false)
                // .appender("kismet")
                .build(log::LevelFilter::Debug),
        )?;

    init_config(config)?;

    Ok(())
}
