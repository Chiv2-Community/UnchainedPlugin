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
        let mut console = Vec::new();
        super::console::ConsoleEncoder::with_patterns(Some("{fn}|{t}|{l}|"), Some("{K(request)}|{m}"))
            .encode(&mut SimpleWriter(&mut console), record).unwrap();
        assert_eq!(console, output);
        let mut default_console = Vec::new();
        super::console::ConsoleEncoder::default()
            .encode(&mut SimpleWriter(&mut default_console), record).unwrap();
        let function = record.key_values().get("__unchained_function".into()).unwrap().to_string();
        assert!(String::from_utf8(default_console).unwrap().contains(&format!("::{function}] ")));
        self.0.lock().unwrap().push(String::from_utf8(output).unwrap());
    }
    fn flush(&self) {}
}

fn named(logger: &Capture) {
    crate::info!(logger: logger, target: "custom", request = 42; "hello {}", 7);
}

#[test]
fn custom_target_filtering_keeps_formatting_lazy_and_user_kv_syntax() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    struct NeverFormat;
    impl std::fmt::Display for NeverFormat {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("filtered record was formatted")
        }
    }
    crate::info!(logger: &logger, target: "disabled", "{}", NeverFormat);
    assert!(logger.0.lock().unwrap().is_empty());
    let request = 42;
    crate::info!(logger: &logger, request; "shorthand");
    crate::info!(logger: &logger, request:? = vec![1, 2]; "debug");
    let lines = logger.0.lock().unwrap();
    assert!(lines[0].ends_with("|INFO|42|shorthand"));
    assert!(lines[1].ends_with("|INFO|[1, 2]|debug"));
}

#[test]
#[cfg(unix)] // This source-tree check runs on the host, not in the Wine harness.
fn all_plugin_diagnostics_use_shared_macros() {
    fn check(directory: &std::path::Path) {
        let mut entries: Vec<_> = std::fs::read_dir(directory).unwrap()
            .map(|entry| entry.unwrap().path()).collect();
        entries.sort();
        for path in entries {
            if path.is_dir() { check(&path); }
            else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                // Split punctuation too, so whitespace cannot bypass the check.
                let tokens: Vec<_> = source.split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                    .filter(|token| !token.is_empty()).collect();
                for statement in source.split(';') {
                    let compact: String = statement.chars().filter(|ch| !ch.is_whitespace()).collect();
                    let facade_import = ["use", "log", "::"].concat();
                    if let Some((_, import)) = compact.split_once(&facade_import) {
                        let imports: Vec<_> = import.split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                            .filter(|token| !token.is_empty()).collect();
                        assert!(!imports.iter().any(|name| ["info", "warn", "error", "debug", "trace", "log"].contains(name)),
                            "{}: do not import facade diagnostic macros", path.display());
                    }
                }
                for level in ["info", "warn", "error", "debug", "trace", "log"] {
                    if level == "log" && path.file_name().unwrap() == "macros.rs" { continue; }
                    for (offset, _) in source.match_indices(level) {
                        let after = source[offset + level.len()..].trim_start();
                        if !after.starts_with('!') { continue; }
                        let before = source[..offset].trim_end();
                        let prefix = if path.components().any(|part| part.as_os_str() == "bin") {
                            "UnchainedPlugin::"
                        } else {
                            // Also accepts the hygienic $crate path in macro definitions.
                            "crate::"
                        };
                        assert!(before.ends_with(prefix),
                            "{}: diagnostic must use {prefix}{level}!", path.display());
                    }
                    assert!(level == "log" || !tokens.windows(2).any(|pair| pair == ["log", level]),
                        "{}: do not import facade diagnostic macros", path.display());
                }
            }
        }
    }
    // file! is absolute in the external harness and relative in the plugin.
    let source = std::path::Path::new(file!());
    check(source.parent().unwrap().parent().unwrap());
    check(&source.parent().unwrap().parent().unwrap().parent().unwrap().join("sleuth_macros/src"));
}
struct Example;
impl Example {
    fn method<T>(&self, logger: &Capture, _: T) {
        crate::warn!(logger: logger, "method");
    }
    async fn asynchronous(&self, logger: &Capture) {
        crate::error!(logger: logger, "async");
        (|| crate::trace!(logger: logger, "async closure"))();
        async { crate::info!(logger: logger, "async block"); }.await;
    }
}

#[test]
fn actual_macros_capture_named_functions_and_preserve_records() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    named(&logger);
    Example.method(&logger, 7);
    (|| (|| crate::debug!(logger: &logger, "closure"))())();
    // This future has no suspension points; poll it without an executor dependency.
    let mut future = std::pin::pin!(Example.asynchronous(&logger));
    struct Noop;
    impl std::task::Wake for Noop { fn wake(self: std::sync::Arc<Self>) {} }
    let waker = std::task::Waker::from(std::sync::Arc::new(Noop));
    assert!(std::future::Future::poll(future.as_mut(), &mut std::task::Context::from_waker(&waker)).is_ready());
    let lines = logger.0.lock().unwrap();
    assert_eq!(lines[0], "named|custom|INFO|42|hello 7");
    assert!(lines[1].starts_with("method|"));
    assert!(lines[2].starts_with("actual_macros_capture_named_functions_and_preserve_records|"));
    assert!(lines[3].starts_with("asynchronous|"));
    assert!(lines[4].starts_with("asynchronous|"));
    assert!(lines[5].starts_with("asynchronous|"));
    assert_eq!(lines.len(), 6);
}

#[test]
fn function_alias_uses_builtin_escaping_nesting_width_and_missing_fallback() {
    for (pattern, expected) in [
        ("{{fn}}", "{fn}"), (r"\{fn\}", "{fn}"), ("{{{fn}}}", "{???}"),
        ("{fn}|{fn}", "???|???"), ("{fn:>5}", "  ???"),
        ("{({fn}):>5}", "  ???"), ("{fn:.2}", "??"),
    ] {
        let mut output = Vec::new();
        PatternEncoder::new(pattern).encode(&mut SimpleWriter(&mut output), &Record::builder().build()).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), expected, "{pattern}");
    }
}

#[test]
fn disabled_levels_do_not_evaluate_arguments_or_structured_values() {
    // Isolate the global max-level change from the other macro test.
    const CHILD: &str = "UNCHAINED_LOG_LAZY_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "logging::tests::disabled_levels_do_not_evaluate_arguments_or_structured_values"])
            .env(CHILD, "1").output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
        return;
    }
    log::set_max_level(log::LevelFilter::Off);
    let calls = std::cell::Cell::new(0);
    let expensive = || { calls.set(calls.get() + 1); 42 };
    crate::info!(request = expensive(); "{}", expensive());
    assert_eq!(calls.get(), 0);
    log::set_max_level(log::LevelFilter::Debug);
    crate::trace!("{}", expensive());
    assert_eq!(calls.get(), 0);
}
