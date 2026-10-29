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
fn function_flag_always_prefixes_the_message() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    sinfo!(logger: &logger, target: "custom", fn; "hello {}", 7);
    swarn!(logger: &logger, fn; "warn");
    serror!(logger: &logger, fn; "error");
    sdebug!(logger: &logger, fn; "debug");
    strace!(logger: &logger, fn; "trace");
    let lines = logger.0.lock().unwrap();
    assert_eq!(lines.len(), 5);
    for (line, (level, message)) in lines.iter().zip([
        ("INFO", "hello 7"), ("WARN", "warn"), ("ERROR", "error"),
        ("DEBUG", "debug"), ("TRACE", "trace"),
    ]) {
        assert!(line.starts_with("function_flag_always_prefixes_the_message|"));
        assert!(line.contains(&format!("|{level}|")));
        assert!(line.ends_with(&format!("[function_flag_always_prefixes_the_message] {message}")));
    }
    assert!(lines[0].contains("|custom|"));
}

#[test]
fn function_flag_ignores_encoder_patterns_and_leaves_f_fields_ordinary() {
    #[derive(Default)]
    struct Messages(Mutex<Vec<(String, Option<String>)>>);
    impl Log for Messages {
        fn enabled(&self, _: &Metadata<'_>) -> bool { true }
        fn log(&self, record: &Record<'_>) {
            let mut output = Vec::new();
            PatternEncoder::new("{m}").encode(&mut SimpleWriter(&mut output), record).unwrap();
            self.0.lock().unwrap().push((String::from_utf8(output).unwrap(),
                record.key_values().get("f".into()).map(|value| value.to_string())));
        }
        fn flush(&self) {}
    }
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Messages::default();
    (|| sinfo!(logger: &logger, fn; "message only"))();
    let f = 42;
    sinfo!(logger: &logger, f; "shorthand");
    sinfo!(logger: &logger, f = 7; "explicit");
    assert_eq!(*logger.0.lock().unwrap(), [
        ("[function_flag_ignores_encoder_patterns_and_leaves_f_fields_ordinary] message only".into(), None),
        ("shorthand".into(), Some("42".into())),
        ("explicit".into(), Some("7".into())),
    ]);
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
    sinfo!(logger: &logger, target: "disabled", "{}", NeverFormat);
    sinfo!(logger: &logger, target: "disabled", fn; "{}", NeverFormat);
    assert!(logger.0.lock().unwrap().is_empty());
    let request = 42;
    sinfo!(logger: &logger, request; "shorthand");
    sinfo!(logger: &logger, request:? = vec![1, 2]; "debug");
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
                assert!(!interpolates_facade_path(&source),
                    "{}: generated diagnostics must use shared macros", path.display());
                // Declarative definitions in this tree end with an unindented brace.
                let mut in_macro = false;
                let mut start = 0;
                let mut macro_definitions = Vec::new();
                let mut position = 0;
                for line in source.split_inclusive('\n') {
                    if line.trim_start().starts_with("macro_rules!") {
                        in_macro = true;
                        start = position;
                    }
                    position += line.len();
                    if in_macro && line.trim_end() == "}" {
                        macro_definitions.push(start..position);
                        in_macro = false;
                    }
                }
                assert!(!in_macro, "{}: unterminated declarative macro", path.display());
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
                for level in ["info", "warn", "error", "debug", "trace", "log", "sinfo", "swarn", "serror", "sdebug", "strace"] {
                    if level == "log" && path.file_name().unwrap() == "macros.rs" { continue; }
                    for (offset, _) in source.match_indices(level) {
                        let after = source[offset + level.len()..].trim_start();
                        if !after.starts_with('!') { continue; }
                        let before = source[..offset].trim_end();
                        if before.ends_with(|ch: char| ch.is_alphanumeric() || ch == '_') { continue; }
                        assert!(level.starts_with('s'),
                            "{}: diagnostic must use the shared s-prefixed macro", path.display());
                        let in_macro = macro_definitions.iter().any(|range| range.contains(&offset));
                        let generated = path.components().any(|part| part.as_os_str() == "sleuth_macros");
                        assert!(shared_macro_scope(before, in_macro, generated),
                            "{}: use unqualified {level}! (declarative definitions: $crate::; proc output: crate::)", path.display());
                    }
                    assert!(level == "log" || level.starts_with('s') || !tokens.windows(2).any(|pair| pair == ["log", level]),
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

#[cfg(unix)]
fn shared_macro_scope(before: &str, in_macro: bool, generated: bool) -> bool {
    if in_macro { before.ends_with("$crate::") }
    else if generated { before.ends_with("crate::") }
    else { !before.ends_with(':') }
}

#[test]
#[cfg(unix)]
fn source_check_enforces_unqualified_calls_and_exported_hygiene() {
    assert!(shared_macro_scope("", false, false));
    for prefix in ["crate::", "UnchainedPlugin::", "$crate::", "log::"] {
        assert!(!shared_macro_scope(prefix, false, false));
    }
    assert!(shared_macro_scope("$crate::", true, false));
    assert!(!shared_macro_scope("crate::", true, false));
    assert!(!shared_macro_scope("", true, false));
    assert!(shared_macro_scope("crate::", false, true));
    assert!(!shared_macro_scope("", false, true));
}

#[cfg(unix)]
fn interpolates_facade_path(source: &str) -> bool {
    let compact: String = source.chars().filter(|ch| !ch.is_whitespace()).collect();
    compact.contains(&["log::", "#"].concat())
}

#[test]
#[cfg(unix)]
fn source_check_catches_interpolated_facade_paths() {
    for prefix in ["::log::", "log :: "] {
        assert!(interpolates_facade_path(&[prefix, "# severity ! (\"failure\")"].concat()));
    }
    assert!(!interpolates_facade_path("crate::#severity!(\"failure\")"));
}

#[test]
#[cfg(unix)]
fn startup_only_dumps_redacted_cli_after_logging_init() {
    let root = std::path::Path::new(file!()).parent().unwrap().parent().unwrap();
    let startup = std::fs::read_to_string(root.join("lib.rs")).unwrap();
    let startup = startup.split_once("let args = tools::cli_args::load_cli()").unwrap().1
        .split_once("// Load build info").unwrap().0;
    let cli = std::fs::read_to_string(root.join("tools/cli_args.rs")).unwrap();
    let declaration = cli.split_once("pub struct CLIArgs").unwrap().0;
    let derive = declaration.rsplit_once("#[derive(").unwrap().1.split_once(")]").unwrap().0;
    assert!(!derive.contains("Debug"), "CLIArgs must use redacting Debug, not derive");
    assert!(cli.contains("impl std::fmt::Debug for CLIArgs"));
    let loader = cli.split_once("pub fn load_cli()").unwrap().1
        .split_once("#[cfg(test)]").unwrap().0;
    assert!(safe_cli_logging(startup, loader), "Only the post-init redacted Debug dump is allowed");
    let facade = "log::";
    for raw in ["sdebug!(\"{:?}\", std::env::args());".to_string(),
        [facade, "debug", "!(\"{parsed:?}\");"].concat(),
        "strace!(\"{args:?}\");".to_string(), "sinfo!(\"{raw:?}\");".to_string(),
        "serror!(\"{raw:?}\");".to_string()] {
        assert!(!safe_cli_logging(&format!("{startup}\n{raw}"), loader));
        assert!(!safe_cli_logging(startup, &format!("{loader}\n{raw}")));
    }
    assert!(!safe_cli_logging("sdebug!(fn; \"CLI args: {:?}\", cli_args());", loader));
}

#[cfg(unix)]
fn safe_cli_logging(startup: &str, loader: &str) -> bool {
    let compact = |source: &str| source.chars().filter(|ch| !ch.is_whitespace()).collect::<String>();
    let startup = compact(startup);
    let dump = "sdebug!(fn;\"CLIargs:{:?}\",cli_args());";
    let Some((before, after)) = startup.split_once(dump) else { return false; };
    if !before.contains("logging::log4rs::init().expect(") { return false; }
    // Allow the existing initialization error, but no other argument diagnostics.
    let remaining = format!("{before}{after}").replace("serror!(fn;\"CLIargsalreadyinitialized!\");", "");
    for source in [remaining, compact(loader)] {
        for severity in ["debug", "trace", "info", "warn", "error", "log"] {
            if source.contains(&format!("{severity}!")) { return false; }
        }
    }
    true
}
struct Example;
impl Example {
    fn method<T>(&self, logger: &Capture, _: T) {
        swarn!(logger: logger, "method");
    }
    async fn asynchronous(&self, logger: &Capture) {
        serror!(logger: logger, "async");
        (|| strace!(logger: logger, "async closure"))();
        async { sinfo!(logger: logger, "async block"); }.await;
    }
}

#[test]
fn actual_macros_capture_named_functions_and_preserve_records() {
    log::set_max_level(log::LevelFilter::Trace);
    let logger = Capture::default();
    named(&logger);
    Example.method(&logger, 7);
    (|| (|| sdebug!(logger: &logger, "closure"))())();
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
    sinfo!(request = expensive(); "{}", expensive());
    sinfo!(fn; "{}", expensive());
    assert_eq!(calls.get(), 0);
    log::set_max_level(log::LevelFilter::Debug);
    strace!("{}", expensive());
    strace!(fn; "{}", expensive());
    assert_eq!(calls.get(), 0);
}
