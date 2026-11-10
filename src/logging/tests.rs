use super::log4rs::pattern::PatternEncoder;
use log::{Log, Metadata, Record};
use log4rs::encode::{Encode, writer::simple::SimpleWriter};
use std::sync::Mutex;

#[derive(Default)]
struct Capture(Mutex<Vec<String>>);
impl Log for Capture {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool { metadata.target() != "disabled" }
    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) { return; }
        assert!(record.module_path().unwrap().ends_with("logging::tests"));
        assert!(record.file().unwrap().ends_with("tests.rs"));
        assert!(record.line().unwrap() > 0);
        let mut output = Vec::new();
        PatternEncoder::new("{fn}|{t}|{l}|{K(request)}|{m}")
            .encode(&mut SimpleWriter(&mut output), record).unwrap();
        self.0.lock().unwrap().push(String::from_utf8(output).unwrap());
    }
    fn flush(&self) {}
}

fn named(logger: &Capture) {
    sinfo!(logger: logger, target: "custom", request = 42; "hello {}", 7);
}

#[test]
fn message_only_pattern_omits_function_metadata() {
    #[derive(Default)]
    struct Messages(Mutex<Vec<String>>);
    impl Log for Messages {
        fn enabled(&self, _: &Metadata<'_>) -> bool { true }
        fn log(&self, record: &Record<'_>) {
            let mut output = Vec::new();
            PatternEncoder::new("{m}").encode(&mut SimpleWriter(&mut output), record).unwrap();
            self.0.lock().unwrap().push(String::from_utf8(output).unwrap());
        }
        fn flush(&self) {}
    }
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Messages::default();
    sinfo!(logger: &logger, "message only");
    assert_eq!(*logger.0.lock().unwrap(), ["message only"]);
}

#[test]
fn filtered_calls_do_not_format_messages() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    struct NeverFormat;
    impl std::fmt::Display for NeverFormat {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("filtered record was formatted")
        }
    }
    sinfo!(logger: &logger, target: "disabled", "{}", NeverFormat);
    assert!(logger.0.lock().unwrap().is_empty());
}

#[test]
fn logger_expression_is_evaluated_once() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    let evaluations = std::cell::Cell::new(0);
    let get_logger = || -> &dyn Log {
        evaluations.set(evaluations.get() + 1);
        &logger
    };
    sinfo!(logger: get_logger(), ("once"), palette("info"));
    assert_eq!(evaluations.get(), 1);
}

#[test]
fn filtered_inline_calls_do_not_evaluate_segments() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    fn never<T>() -> T { panic!("filtered argument evaluated") }
    serror!(logger: &logger, target: "disabled", ("{}", never::<String>()),
        palette(never::<&str>()), ("never"));
    assert!(logger.0.lock().unwrap().is_empty());
}

#[test]
fn structured_values_support_shorthand_and_debug_formatting() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    let request = 42;
    sinfo!(logger: &logger, request; "shorthand");
    sinfo!(logger: &logger, request:? = vec![1, 2]; "debug");
    let lines = logger.0.lock().unwrap();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("|INFO|42|shorthand"));
    assert!(lines[1].ends_with("|INFO|[1, 2]|debug"));
}

#[test]
fn chat_attributes_reach_json_output() {
    #[derive(Default)]
    struct JsonCapture(Mutex<Vec<u8>>);
    impl Log for JsonCapture {
        fn enabled(&self, _: &Metadata<'_>) -> bool { true }
        fn log(&self, record: &Record<'_>) {
            log4rs::encode::json::JsonEncoder::new()
                .encode(&mut SimpleWriter(&mut *self.0.lock().unwrap()), record).unwrap();
        }
        fn flush(&self) {}
    }
    log::set_max_level(log::LevelFilter::Trace);
    let logger = JsonCapture::default();
    sinfo!(logger: &logger, chat = "TeamSay", team = "Mason"; "hello");
    let output: serde_json::Value = serde_json::from_slice(&logger.0.lock().unwrap()).unwrap();
    assert_eq!(output["attributes"]["chat"], "TeamSay");
    assert_eq!(output["attributes"]["team"], "Mason");
}

#[test]
fn inline_segments_keep_plain_messages_and_metadata() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    sinfo!(logger: &logger, target: "custom", ("hello {count}", count = 7), palette("WARN"), " world", palette(), "!");
    let lines = logger.0.lock().unwrap();
    assert_eq!(*lines, ["inline_segments_keep_plain_messages_and_metadata|custom|INFO||hello 7 world!"]);
}

#[test]
fn inline_syntax_preserves_grouped_formats_literals_and_palette_only_records() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    let request = 42;
    sinfo!(logger: &logger, request; ("{}", String::from("one")), "{request}",);
    sinfo!(logger: &logger, palette("warn"), palette(),);
    let count = 7;
    sinfo!(logger: &logger, palette("warn"), "before {count}", palette(), "after",);
    sinfo!(logger: &logger, ("before {}", count), palette("warn"), "after");
    let lines = logger.0.lock().unwrap();
    assert_eq!(lines.len(), 4);
    for (line, expected) in lines.iter().zip([
        "|INFO|42|one42", "|INFO||", "before 7after", "before 7after",
    ]) {
        assert!(line.ends_with(expected), "{line}");
    }
}

#[test]
fn inline_calls_borrow_owned_loggers() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    sinfo!(logger: logger, ("info"), palette("info"), (" tail"));
    sinfo!(logger: logger, "ordinary reuse");
    let lines = logger.0.lock().unwrap();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("info tail"));
    assert!(lines[1].ends_with("ordinary reuse"));
}

#[test]
fn inline_calls_borrow_logger_fields_through_shared_receivers() {
    struct Owner(Capture);
    impl Owner {
        fn emit(&self) {
            sinfo!(logger: self.0, ("field"), palette("info"), (" tail"));
            sinfo!(logger: self.0, "ordinary reuse");
        }
    }
    log::set_max_level(log::LevelFilter::Trace);
    let owner = Owner(Capture::default());
    owner.emit();
    let lines = owner.0.0.lock().unwrap();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("field tail"));
    assert!(lines[1].ends_with("ordinary reuse"));
}

#[test]
fn inline_formatting_is_stable_across_multiple_encoders() {
    struct Counter(std::cell::Cell<usize>);
    impl std::fmt::Display for Counter {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let next = self.0.get() + 1;
            self.0.set(next);
            write!(f, "界{next}")
        }
    }
    struct Logger;
    impl Log for Logger {
        fn enabled(&self, _: &Metadata<'_>) -> bool { true }
        fn log(&self, record: &Record<'_>) {
            for _ in 0..2 {
                let mut output = Vec::new();
                PatternEncoder::new("{m}").encode(&mut SimpleWriter(&mut output), record).unwrap();
                assert_eq!(output, "界1tail".as_bytes());
            }
            let colors = record.key_values().get("inline_colors".into()).unwrap();
            assert_eq!(serde_json::from_str::<serde_json::Value>(colors.to_borrowed_str().unwrap()).unwrap(),
                serde_json::json!([{"utf8_offset": 4, "palette": "warn"}]));
        }
        fn flush(&self) {}
    }
    log::set_max_level(log::LevelFilter::Trace);
    let counter = Counter(std::cell::Cell::new(0));
    sinfo!(logger: Logger, ("{}", counter), palette("warn"), "tail");
    assert_eq!(counter.0.get(), 1);
}

#[test]
fn ordinary_calls_do_not_create_inline_state() {
    struct Plain(Capture);
    impl Log for Plain {
        fn enabled(&self, metadata: &Metadata<'_>) -> bool { self.0.enabled(metadata) }
        fn log(&self, record: &Record) {
            assert!(record.key_values().get("inline_colors".into()).is_none());
            self.0.log(record);
        }
        fn flush(&self) {}
    }
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Plain(Capture::default());
    sinfo!(logger: &logger, "ordinary {}", 7);
    let lines = logger.0.0.lock().unwrap();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].ends_with("ordinary 7"));
}

struct Example;
impl Example {
    fn method<T>(&self, logger: &Capture, _: T) {
        swarn!(logger: logger, "method");
    }
}

#[test]
fn named_function_preserves_record_fields() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    named(&logger);
    assert_eq!(*logger.0.lock().unwrap(), ["named|custom|INFO|42|hello 7"]);
}

#[test]
fn methods_capture_leaf_name() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    Example.method(&logger, 7);
    assert!(logger.0.lock().unwrap()[0].starts_with("method|"));
}

#[test]
fn closures_capture_enclosing_function() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    (|| sdebug!(logger: &logger, "closure"))();
    assert!(logger.0.lock().unwrap()[0].starts_with("closures_capture_enclosing_function|"));
}
