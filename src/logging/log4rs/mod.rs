use anyhow::Context;
use log4rs::config::{Config, Deserializers, RawConfig};
use console::ConsoleAppenderDeserializer;
use palette::PaletteEncoderDeserializer;
use pattern::PatternEncoderDeserializer;

mod console;
pub(crate) mod palette;
pub mod pattern;
use std::backtrace::Backtrace;
use std::panic;
use crate::tools::hook_globals::cli_args;
use std::path::PathBuf;

const EMBEDDED_LOG4RS_YAML: &str = include_str!("log4rs.default.yaml");
const EMBEDDED_LOG4RS_DEFAULT_NAME: &str = "log4rs.default.yaml";
const UNCHAINED_LOG_PATH_PLACEHOLDER: &str = "__UNCHAINED_LOG_PATH__";
const KISMET_LOG_PATH_PLACEHOLDER: &str = "__KISMET_LOG_PATH__";

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

        serror!(
            "PANIC at {}: {}\nStack Backtrace:\n{}",
            location, msg, backtrace
        );
    }));
}

pub fn init() -> anyhow::Result<()> {
    let cli = cli_args();
    let config_source_name = cli.log4rs_yaml.as_deref().unwrap_or(EMBEDDED_LOG4RS_DEFAULT_NAME);
    let yaml = load_logger_yaml(cli.log4rs_yaml.as_deref())?;
    let resolved_yaml = apply_logger_placeholders(&yaml);
    log4rs::init_config(config_from_yaml(&resolved_yaml, config_source_name)?)
        .with_context(|| format!("Failed to initialize logger from config {}", config_source_name))?;
    setup_panic_logger();
    crate::logging::console::report_attachment_failure();

    Ok(())
}

fn load_logger_yaml(override_path: Option<&str>) -> anyhow::Result<String> {
    match override_path {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read logger config file: {}", path)),
        None => Ok(EMBEDDED_LOG4RS_YAML.to_string()),
    }
}

fn config_from_yaml(yaml: &str, config_source_name: &str) -> anyhow::Result<Config> {
    let raw_config: RawConfig = serde_yaml::from_str(yaml).with_context(|| {
        format!(
            "Failed to parse logger config {}",
            config_source_name
        )
    })?;
    let mut deserializers = Deserializers::default();
    register(&mut deserializers);
    let (appenders, errors) = raw_config.appenders_lossy(&deserializers);
    anyhow::ensure!(errors.is_empty(), "Invalid logger config {}: {}", config_source_name, errors);
    Config::builder()
        .appenders(appenders)
        .loggers(raw_config.loggers())
        .build(raw_config.root())
        .with_context(|| format!("Invalid logger config {}", config_source_name))
}

/// Register pattern extensions and the palette encoder.
pub fn register(deserializers: &mut Deserializers) {
    deserializers.insert("palette", PaletteEncoderDeserializer);
    deserializers.insert("console", ConsoleAppenderDeserializer);
    deserializers.insert("pattern", PatternEncoderDeserializer);
}

fn apply_logger_placeholders(yaml: &str) -> String {
    let suffix = cli_args()
        .saved_dir_suffix
        .as_deref()
        .map(|value| format!("_{}", value))
        .unwrap_or_default();

    let log_dir = std::env::var("LOCALAPPDATA").ok()
        .map(|base| PathBuf::from(base).join(format!(r"Chivalry 2\Saved{suffix}")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Logs");

    if let Err(error) = std::fs::create_dir_all(&log_dir) {
        eprintln!(
            "[{}] Failed to create log directory {}: {}",
            module_path!(),
            log_dir.display(),
            error
        );
    }

    let unchained_log_path = yaml_escape_double_quoted(
        &log_dir.join("unchained.log").to_string_lossy()
    );
    let kismet_log_path = yaml_escape_double_quoted(
        &log_dir.join("kismet.log").to_string_lossy()
    );

    yaml.replace(UNCHAINED_LOG_PATH_PLACEHOLDER, &unchained_log_path)
        .replace(KISMET_LOG_PATH_PLACEHOLDER, &kismet_log_path)
}

fn yaml_escape_double_quoted(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::{Level, Log, Metadata, Record};
    use log4rs::encode::{Encode, EncoderConfig, writer::simple::SimpleWriter};

    #[test]
    fn inline_palette_switches_color_console_and_preserve_plain_file_messages() {
        let path = std::env::temp_dir().join(format!("inline-palette-route-{}.jsonl", std::process::id()));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "--nocapture",
                &format!("{}::inline_palette_child", module_path!().split_once("::").unwrap().1)])
            .env("UNCHAINED_TEST_INLINE_FILE", &path)
            .env("CLICOLOR_FORCE", "1").env_remove("NO_COLOR").output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("ordinary"));
        #[cfg(unix)]
        {
            assert!(text.contains("\x1b[0;38;2;18;52;86mafter"), "{text:?}");
            assert!(text.contains("\x1b[0;38;2;171;205;239mfirst line\r\n"), "{text:?}");
            assert!(text.contains("\x1b[0;38;2;254;220;186msecond 7\r\n"), "{text:?}");
        }
        let file = std::fs::read_to_string(&path).unwrap();
        let records: Vec<serde_json::Value> = file.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(records.len(), 4);
        assert_eq!(records[0]["message"], "ordinary");
        assert!(records[0]["attributes"].get("inline_colors").is_none());
        assert_eq!(records[1]["message"], "beforeafterend");
        let colors: serde_json::Value = serde_json::from_str(records[1]["attributes"]["inline_colors"].as_str().unwrap()).unwrap();
        assert_eq!(colors, serde_json::json!([
            {"utf8_offset": 6, "palette": "accent"}, {"utf8_offset": 11, "palette": null}
        ]));
        assert_eq!(records[2]["message"], "onetwo");
        assert!(records[2]["attributes"].get("inline_colors").is_none());
        assert_eq!(records[3]["message"], "first line\r\nsecond 7\r\n");
        assert!(!file.contains("\\u001b"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "subprocess fixture for inline palette messages"]
    fn inline_palette_child() {
        let path = std::env::var("UNCHAINED_TEST_INLINE_FILE").unwrap();
        let yaml = format!(r##"
appenders:
  console:
    kind: console
    encoder:
      kind: palette
      pattern: '{{m}}{{n}}'
      palette: {{accent: '#123456', first: '#abcdef', second: '#fedcba'}}
  file:
    kind: file
    path: {}
    encoder: {{kind: json}}
root:
  level: info
  appenders: [console, file]
"##, serde_json::to_string(&path).unwrap());
        log4rs::init_config(config_from_yaml(&yaml, "inline-palette").unwrap()).unwrap();
        sinfo!("ordinary");
        sinfo!(("before"), palette("accent"), "after", palette(), "end");
        sinfo!("onetwo");
        sinfo!(palette("first"), "first line\r\n", palette("second"), ("second {}", 7), "\r\n");
        log::logger().flush();
    }

    fn render_encoder(registry: &Deserializers, yaml: &str, record: &Record) -> Vec<u8> {
        let config: EncoderConfig = serde_yaml::from_str(yaml).unwrap();
        let encoder: Box<dyn Encode> = registry.deserialize(&config.kind, config.config)
            .unwrap_or_else(|error| panic!("{yaml}: {error}"));
        let mut output = Vec::new();
        encoder.encode(&mut SimpleWriter(&mut output), record).unwrap();
        output
    }

    fn console_config(encoder: &str) -> anyhow::Result<Config> {
        let encoder: String = encoder.lines().map(|line| format!("      {line}\n")).collect();
        config_from_yaml(&format!("appenders:\n  console:\n    kind: console\n    encoder:\n{encoder}root:\n  level: info\n  appenders: [console]\n"), "console-test")
    }

    #[test]
    fn json_encoder_respects_configured_stdio_and_file_sinks() {
        let directory = std::env::temp_dir().join(format!("unchained-json-sinks-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&directory).unwrap();
        for (target, kind) in [("stdout", "file"), ("stderr", "rolling_file"), ("file-only", "file")] {
            let path = directory.join(format!("{target}.jsonl"));
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args(["--ignored", "--exact", "--nocapture",
                &format!("{}::json_sink_child", module_path!().split_once("::").unwrap().1)])
                .env("UNCHAINED_TEST_JSON_TARGET", target)
                .env("UNCHAINED_TEST_JSON_FILE", &path)
                .env("UNCHAINED_TEST_JSON_FILE_KIND", kind);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000); // No console; inherit redirected stdio.
            }
            let output = command.output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            let read_records = |bytes: &[u8]| -> Vec<serde_json::Value> {
                String::from_utf8_lossy(bytes).lines()
                    .filter(|line| line.starts_with('{'))
                    .map(|line| serde_json::from_str(line).unwrap()).collect()
            };
            let file = read_records(&std::fs::read(&path).unwrap());
            assert_eq!(file.len(), 3);
            assert_eq!(file[0]["message"], "[json_sink_child] hello 世界\nsecond line");
            assert_eq!(file[1]["attributes"]["request"], "42");
            assert_eq!(file[0]["attributes"]["enclosing_function"], "json_sink_child");
            assert_eq!(file[0]["module_path"], module_path!());
            assert!(file[0]["line"].as_u64().unwrap() > 0);
            assert!(file[0]["thread_id"].as_u64().unwrap() > 0);
            assert!(file[0]["time"].as_str().unwrap().contains('T'));
            assert_eq!(file[1]["message"].as_str().unwrap().len(), 20 * 1024);
            assert_eq!(file[2]["message"], "fixture line one\nfixture line two");
            let stdout = read_records(&output.stdout);
            let stderr = read_records(&output.stderr);
            if target == "file-only" {
                assert!(stdout.is_empty() && stderr.is_empty());
            } else {
                let (selected, other) = if target == "stdout" { (stdout, stderr) } else { (stderr, stdout) };
                assert!(other.is_empty());
                assert_eq!(selected.len(), file.len());
                for (console, file) in selected.iter().zip(&file) {
                    assert_eq!(console["message"], file["message"]);
                    assert_eq!(console["attributes"], file["attributes"]);
                }
            }
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    #[ignore = "subprocess fixture for configured JSON sinks"]
    fn json_sink_child() {
        let target = std::env::var("UNCHAINED_TEST_JSON_TARGET").unwrap();
        #[cfg(windows)]
        let before = unsafe { windows::Win32::System::Console::GetConsoleWindow() };
        if target == "stderr" { crate::logging::console::disable_console_spawning(); }
        else {
            crate::logging::console::attach_stdio();
            assert!(!crate::logging::console::console_spawning_disabled());
            crate::logging::console::disable_console_spawning();
        }
        let file = std::env::var("UNCHAINED_TEST_JSON_FILE").unwrap();
        let kind = std::env::var("UNCHAINED_TEST_JSON_FILE_KIND").unwrap();
        let policy = if kind == "rolling_file" {
            "policy: {kind: compound, trigger: {kind: size, limit: 1048576}, roller: {kind: delete}}"
        } else { "" };
        let appenders = if target == "file-only" { "[file]" } else { "[console, file]" };
        let console_target = if target == "file-only" { "stdout" } else { &target };
        let config = config_from_yaml(&format!(r#"
appenders:
  console:
    kind: console
    target: {console_target}
    encoder: {{kind: json}}
  file:
    kind: {kind}
    path: "{}"
    {policy}
    encoder: {{kind: json}}
root:
  level: info
  appenders: {appenders}
"#, yaml_escape_double_quoted(&file)), "json-sink-test").unwrap();
        let _handle = log4rs::init_config(config).unwrap();
        sinfo!(fn; "hello 世界\nsecond line");
        sinfo!(request = 42; "{}", "x".repeat(20 * 1024));
        sinfo!("fixture line one\nfixture line two");
        log::logger().flush();
        #[cfg(windows)]
        assert_eq!(before, unsafe { windows::Win32::System::Console::GetConsoleWindow() });
    }

    #[cfg(unix)]
    #[test]
    fn console_color_policy_at_real_output_boundary() {
        let output = std::process::Command::new("python3")
            .args(["-c", include_str!("test_console_policy.py")])
            .arg(std::env::current_exe().unwrap())
            .arg(format!("{}::console_policy_child", module_path!().split_once("::").unwrap().1))
            .output().unwrap();
        assert!(output.status.success(), "{}\n{}",
            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "subprocess fixture for console_color_policy_at_real_output_boundary"]
    fn console_policy_child() {
        let target = std::env::var("UNCHAINED_TEST_CONSOLE_TARGET").unwrap();
        let custom = if std::env::var_os("UNCHAINED_TEST_CUSTOM_PALETTE").is_some() {
            "palette: {error: '#112233', warn: '#223344', info: '#334455', debug: '#445566', trace: '#556677', error_body: null, warn_body: null, info_body: null, debug_body: '#445566', trace_body: '#556677'}"
        } else { "" };
        let config = config_from_yaml(&format!(r#"
appenders:
  console:
    kind: console
    target: {target}
    encoder:
      kind: palette
      {custom}
root:
  level: trace
  appenders: [console]
"#), "console-policy-child").unwrap();
        let logger = log4rs::Logger::new(config);
        for level in [Level::Error, Level::Warn, Level::Info, Level::Debug, Level::Trace] {
            logger.log(&log::Record::builder().level(level).module_path(Some("policy_child"))
                .args(format_args!("boundary message")).build());
        }
    }

    #[test]
    fn embedded_configuration_preserves_patterns_and_filters() {
        let mut yaml: serde_yaml::Value = serde_yaml::from_str(EMBEDDED_LOG4RS_YAML).unwrap();
        assert_eq!(yaml["appenders"]["console"]["encoder"]["pattern"], palette::DEFAULT_PATTERN);
        for name in ["file", "kismet"] {
            let pattern = yaml["appenders"][name]["encoder"]["pattern"].as_str().unwrap();
            assert!(pattern.contains("{l:5}"));
            assert!(pattern.contains("{M}"));
            assert!(!pattern.contains("{t}"));
        }
        // Exercise the shipped console config without opening log files.
        yaml["appenders"].as_mapping_mut().unwrap()
            .retain(|name, _| name.as_str() == Some("console"));
        yaml["root"]["appenders"] = serde_yaml::from_str("[console]").unwrap();
        let config = config_from_yaml(&serde_yaml::to_string(&yaml).unwrap(), "embedded").unwrap();
        let logger = log4rs::Logger::new(config);
        assert!(logger.enabled(&Metadata::builder()
            .target("UnchainedPlugin::logging::log4rs").level(Level::Debug).build()));
        assert!(!logger.enabled(&Metadata::builder()
            .target("UnchainedPlugin::logging::log4rs").level(Level::Trace).build()));
        assert!(!logger.enabled(&Metadata::builder()
            .target("serenity::client").level(Level::Info).build()));
    }

    #[test]
    fn registered_encoders_accept_short_and_full_module_fields() {
        let mut deserializers = Deserializers::default();
        register(&mut deserializers);
        for yaml in [
            "kind: pattern\npattern: '{sm}|{M}|{t}|{fn}'",
            "kind: palette\npattern: '{sm}|{M}|{t}|{fn}'",
        ] {
            let output = render_encoder(&deserializers, yaml, &Record::builder()
                .module_path(Some("UnchainedPlugin::messages")).target("filter::target").build());
            assert_eq!(String::from_utf8(output).unwrap(), "UP::messages|UnchainedPlugin::messages|filter::target|???");
        }
    }

    #[test]
    fn registered_palette_encoder_preserves_directive_boundary_escapes() {
        let mut deserializers = Deserializers::default();
        register(&mut deserializers);
        for (pattern, expected) in [
            ("{palette(error)}}}{m}", "}message"),
            ("{{{palette(error)}{l}{palette()}}}", "{INFO}"),
            ("{palette(error)}}}}}{m}", "}}message"),
            (r"{palette(error)}\}{m}", "}message"),
            ("{palette(error)}{{{m}}}", "{message}"),
            ("{palette(error)}{palette()}}}{m}", "}message"),
            ("{palette({l})}}}{m}", "}message"),
            ("{palette(a}}b)}{m}", "message"),
            ("{palette(a{{b)}{m}", "message"),
            ("{palette(a))}}b)}}}{m}", "}message"),
            (r"{palette(a\)\}b)}}}{m}", "}message"),
            (r"\{palette\(missing\)\}{palette()}{h({m})}", "{palette(missing)}message"),
        ] {
            let yaml = format!("kind: palette\npattern: '{pattern}'\npalette: {{error: null, warn: null, info: null, debug: null, trace: null, 'a}}b': null, 'a{{b': null, 'a)}}b': null}}");
            let output = render_encoder(&deserializers, &yaml, &Record::builder()
                .level(Level::Info).args(format_args!("message")).build());
            assert_eq!(String::from_utf8(output).unwrap(), expected, "{pattern}");
        }
    }

    #[test]
    fn registered_palette_encoder_preserves_native_format_fills() {
        let mut deserializers = Deserializers::default();
        register(&mut deserializers);
        let record = Record::builder().level(Level::Info).args(format_args!("message")).build();
        for (formatter, fill, align) in [
            ("m", '{', '>'), ("m", '}', '<'),
            ("({m})", '{', '<'), ("h({m})", '}', '>'),
            ("m", '🦀', '>'), ("({m})", '🦀', '<'),
            ("({m})", '\\', '<'), ("m", '(', '<'), ("h({m})", ')', '>'),
        ] {
            let native = format!("{{{formatter}:{fill}{align}20}}tail");
            let mut expected = Vec::new();
            log4rs::encode::pattern::PatternEncoder::new(&native)
                .encode(&mut SimpleWriter(&mut expected), &record).unwrap();
            let pattern = format!("{{palette(error)}}{{{formatter}:{fill}{align}20}}{{palette()}}tail");
            let output = render_encoder(&deserializers, &format!("kind: palette\npattern: '{pattern}'"), &record);
            assert_eq!(output, expected, "{pattern}");
        }
        console_config("kind: palette\npattern: '{m:{>20}{palette(error)}tail'").unwrap();
    }

    #[test]
    fn registered_palette_names_accept_native_level_format_fills() {
        let mut deserializers = Deserializers::default();
        register(&mut deserializers);
        for (field, fill, align) in [
            ("l", '{', '>'), ("level", '}', '<'), ("level", '🦀', '>'),
            ("l", '\\', '<'), ("l", '(', '>'), ("level", ')', '<'),
        ] {
            let name = format!("{{{field}:{fill}{align}8}}");
            let mut palette = std::collections::BTreeMap::<String, Option<String>>::new();
            for level in [Level::Error, Level::Warn, Level::Info, Level::Debug, Level::Trace] {
                let mut key = Vec::new();
                log4rs::encode::pattern::PatternEncoder::new(&name).encode(&mut SimpleWriter(&mut key),
                    &Record::builder().level(level).build()).unwrap();
                palette.insert(String::from_utf8(key).unwrap(), None);
            }
            let pattern = format!("{{palette({name})}}{{m}}{{palette()}}}}}}");
            let mut yaml: serde_yaml::Value = serde_yaml::from_str(&format!("kind: palette\npattern: '{pattern}'")).unwrap();
            yaml["palette"] = serde_yaml::to_value(palette).unwrap();
            let output = render_encoder(&deserializers, &serde_yaml::to_string(&yaml).unwrap(),
                &Record::builder().level(Level::Info).args(format_args!("message")).build());
            assert_eq!(output, b"message}", "{pattern}");
        }
    }

    #[test]
    fn invalid_palette_configuration_is_rejected() {
        for options in [
            "palette: {info: red}", "palette: {info: '#12345'}", "palette: {info: '#gg2233'}",
            "palette: {info: '#é2233'}", "palette: {info: '#+f3344'}", "palette: {info: 123}",
            "palette: {'': null}", "palette: {info: null, INFO: null}", "unknown: true",
            "pattern: '{palette(missing)}'", "pattern: '{palette({l}_missing)}'",
            "pattern: '{palette({K(user)})}'", "pattern: '{h({palette(error)}text)}'",
            "pattern: '{palette(error)'", "pattern: '{palette(error}'",
            "pattern: '{palette({l})}hello'\npalette: {info: '#112233'}",
            "pattern: '{h({m:{>20}{palette(error)})}'", "pattern: '{h({m:}>20}{palette()})}'",
            "pattern: '{h(text:{>20}{palette(error)})}'", "pattern: '{palette({m:{>20})}'",
            "pattern: '{palette({K(user):}>20})}'", "pattern: '{palette({l:{>8}_missing)}'",
        ] {
            assert!(console_config(&format!("kind: palette\n{options}")).is_err(), "{options}");
        }
    }

    #[test]
    fn file_appenders_share_console_patterns() {
        use std::sync::Mutex;

        struct TempDir(PathBuf);
        impl Drop for TempDir {
            fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
        }
        let dir = TempDir(std::env::temp_dir().join(format!("unchained-file-pattern-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())));
        std::fs::create_dir(&dir.0).unwrap();

        struct CompareLogger {
            file: log4rs::Logger,
            console: Box<dyn Encode>,
            expected: Mutex<Vec<u8>>,
        }
        impl Log for CompareLogger {
            fn enabled(&self, metadata: &Metadata<'_>) -> bool { self.file.enabled(metadata) }
            fn log(&self, record: &log::Record<'_>) {
                self.console.encode(&mut SimpleWriter(&mut *self.expected.lock().unwrap()), record).unwrap();
                self.file.log(record);
            }
            fn flush(&self) { self.file.flush(); }
        }

        let header = "{l:5}|{sm:>16}|{({fn}):>40}|";
        let body = "{M}|{t}|{f}:{L}|{K(request)}|{{sm}}|{{fn}}|{m}{n}";
        log::set_max_level(log::LevelFilter::Trace);
        let mut deserializers = Deserializers::default();
        register(&mut deserializers);
        for appender_kind in ["file", "rolling_file"] {
            for encoder_kind in ["", "kind: pattern"] {
                let path = dir.0.join("output.log");
                let policy = if appender_kind == "rolling_file" {
                    "policy: {kind: compound, trigger: {kind: size, limit: 1048576}, roller: {kind: delete}}"
                } else { "" };
                let config = config_from_yaml(&format!(r#"
appenders:
  file:
    kind: {appender_kind}
    path: "{}"
    append: false
    {policy}
    encoder:
      {encoder_kind}
      pattern: '{header}{body}'
root:
  level: info
  appenders: [file]
"#, yaml_escape_double_quoted(&path.to_string_lossy())), "file-pattern-test").unwrap();
                let console_config: EncoderConfig = serde_yaml::from_str(&format!(
                    "kind: palette\npattern: '{header}{body}'")).unwrap();
                let logger = CompareLogger {
                    file: log4rs::Logger::new(config),
                    console: deserializers.deserialize(&console_config.kind, console_config.config).unwrap(),
                    expected: Mutex::new(Vec::new()),
                };
                let source_line = line!() + 1;
                sinfo!(logger: &logger, target: "file::filter", request = 42; "literal {{fn}} message");
                logger.flush();
                let expected = logger.expected.lock().unwrap();
                let text = std::str::from_utf8(&expected).unwrap();
                assert!(text.contains("file_appenders_share_console_patterns"), "{text}");
                assert!(text.contains(&format!("|{}|file::filter|{}:{source_line}|42|{{sm}}|{{fn}}|literal {{fn}} message",
                    module_path!(), file!())), "{text}");
                assert_eq!(std::fs::read(&path).unwrap(), *expected, "{appender_kind}, {encoder_kind}");
            }
        }
    }

    #[test]
    fn console_adapter_preserves_builtin_configuration_validation() {
        for options in ["", "target: null", "target: stderr\n    tty_only: true",
            "encoder:\n      pattern: '{l} [{t}] {m}{n}'"] {
            config_from_yaml(&format!("appenders:\n  console:\n    kind: console\n    {options}\nroot:\n  level: info\n  appenders: [console]\n"), "console-options").unwrap();
        }
        for options in ["target: invalid", "unknown: true", "encoder:\n      kind: missing_encoder"] {
            let error = config_from_yaml(&format!("appenders:\n  console:\n    kind: console\n    {options}\nroot:\n  level: info\n  appenders: [console]\n"), "invalid-console-options").unwrap_err();
            assert!(error.to_string().contains("invalid-console-options"));
        }
    }
}
