use anyhow::Context;
use log4rs::config::{Config, Deserializers, RawConfig};
use console::ConsoleAppenderDeserializer;
use palette::PaletteEncoderDeserializer;
use pattern::PatternEncoderDeserializer;

mod console;
pub(crate) mod palette;
pub mod pattern;
mod removals;
use std::backtrace::Backtrace;
use std::panic;
use crate::tools::hook_globals::cli_args;
use std::path::PathBuf;
use std::collections::BTreeSet;

const EMBEDDED_LOG4RS_YAML: &str = include_str!("log4rs.default.yaml");
const EMBEDDED_LOG4RS_DEFAULT_NAME: &str = "log4rs.default.yaml";
const UNCHAINED_LOG_PATH_PLACEHOLDER: &str = "__UNCHAINED_LOG_PATH__";
const KISMET_LOG_PATH_PLACEHOLDER: &str = "__KISMET_LOG_PATH__";
const CHAT_LOG_PATH_PLACEHOLDER: &str = "__CHAT_LOG_PATH__";

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
    init_with_path(cli.log4rs_yaml.as_deref(), cli.saved_dir_suffix.as_deref())
}

fn init_with_path(override_path: Option<&str>, saved_dir_suffix: Option<&str>) -> anyhow::Result<()> {
    let selected = load_logger_yaml(override_path).and_then(|(source, yaml)|
        config_from_yaml(&yaml, source, saved_dir_suffix).map(|config| (source, config)));
    let mut rejected = None;
    let (config_source_name, (config, warnings)) = match selected {
        Ok(config) => config,
        Err(error) => {
            // No logger is available yet; report the complete error chain on stderr.
            eprintln!("[UnchainedPlugin ERROR] Failed to load logging configuration: {error:#}");
            eprintln!("[UnchainedPlugin ERROR] Startup will resume with embedded {EMBEDDED_LOG4RS_DEFAULT_NAME} in 5 seconds.");
            std::thread::sleep(std::time::Duration::from_secs(5));
            rejected = Some(error);
            (EMBEDDED_LOG4RS_DEFAULT_NAME,
                config_from_yaml(EMBEDDED_LOG4RS_YAML, EMBEDDED_LOG4RS_DEFAULT_NAME, saved_dir_suffix)?)
        }
    };
    log4rs::init_config(config)
        .with_context(|| format!("Failed to initialize logger from config {}", config_source_name))?;
    setup_panic_logger();
    if let Some(error) = rejected {
        serror!("Rejected logging configuration: {error:#}. Resumed with embedded {EMBEDDED_LOG4RS_DEFAULT_NAME} after a 5-second pause.");
    }
    for name in warnings {
        swarn!(undefined_palette = name.as_str();
            "Undefined palette reference in a configured encoder: {}", name);
    }
    crate::logging::console::report_attachment_failure();

    Ok(())
}

fn config_from_yaml(yaml: &str, source: &str, saved_dir_suffix: Option<&str>) -> anyhow::Result<(Config, BTreeSet<String>)> {
    let resolved = apply_logger_placeholders(yaml, saved_dir_suffix);
    let yaml = serde_yaml::from_str(&resolved)
        .with_context(|| format!("Failed to parse logger config {source}"))?;
    config_with_warnings(yaml, source)
}

fn load_logger_yaml(override_path: Option<&str>) -> anyhow::Result<(&str, String)> {
    let path = override_path.unwrap_or("log4rs.yaml");
    match std::fs::read_to_string(path) {
        Err(error) if override_path.is_none() && error.kind() == std::io::ErrorKind::NotFound =>
            Ok((EMBEDDED_LOG4RS_DEFAULT_NAME, EMBEDDED_LOG4RS_YAML.to_string())),
        result => result.map(|yaml| (path, yaml))
            .with_context(|| format!("Failed to read logger config file: {}", path)),
    }
}

fn config_with_warnings(mut yaml: serde_yaml::Value, config_source_name: &str) -> anyhow::Result<(Config, BTreeSet<String>)> {
    share_structured_appenders(&mut yaml);
    let warnings = missing_palette_references(&yaml);
    let raw_config: RawConfig = serde_yaml::from_value(yaml)
        .with_context(|| format!("Failed to parse logger config {}", config_source_name))?;
    let mut deserializers = Deserializers::default();
    register(&mut deserializers);
    let (appenders, errors) = raw_config.appenders_lossy(&deserializers);
    anyhow::ensure!(errors.is_empty(), "Invalid logger config {}: {}", config_source_name, errors);
    let config = Config::builder()
        .appenders(appenders)
        .loggers(raw_config.loggers())
        .build(raw_config.root())
        .with_context(|| format!("Invalid logger config {}", config_source_name))?;
    Ok((config, warnings))
}

#[cfg(test)]
fn config_from_value(yaml: serde_yaml::Value, config_source_name: &str) -> anyhow::Result<Config> {
    config_with_warnings(yaml, config_source_name).map(|(config, _)| config)
}

/// Register encoders and the console appender.
pub fn register(deserializers: &mut Deserializers) {
    deserializers.insert("palette", PaletteEncoderDeserializer);
    deserializers.insert("console", ConsoleAppenderDeserializer);
    deserializers.insert("pattern", PatternEncoderDeserializer);
}

fn share_structured_appenders(yaml: &mut serde_yaml::Value) {
    let shared: Vec<_> = yaml["root"]["appenders"].as_sequence().into_iter().flatten()
        .filter(|name| name.as_str().is_some_and(|name| yaml["appenders"][name]["encoder"]["kind"] == "json"))
        .cloned().collect();
    let Some(loggers) = yaml.get_mut("loggers").and_then(serde_yaml::Value::as_mapping_mut) else { return; };
    for (name, logger) in loggers {
        if !name.as_str().is_some_and(|name| name == "chat_log" || name.starts_with("chat_log::"))
            || logger["additive"].as_bool() != Some(false) { continue; }
        let Some(logger) = logger.as_mapping_mut() else { continue; };
        let appenders = logger.entry(serde_yaml::Value::from("appenders"))
            .or_insert_with(|| serde_yaml::Value::Sequence(Vec::new()));
        if let Some(appenders) = appenders.as_sequence_mut() {
            for name in &shared {
                if !appenders.contains(name) { appenders.push(name.clone()); }
            }
        }
    }
}

fn missing_palette_references(yaml: &serde_yaml::Value) -> BTreeSet<String> {
    let references: BTreeSet<_> = inventory::iter::<crate::logging::inline::PaletteReference>
        .into_iter().map(|reference| reference.0.to_ascii_lowercase())
        .filter(|name| !name.is_empty()).collect();
    let encoders = yaml["appenders"].as_mapping().into_iter()
        .flat_map(|appenders| appenders.values())
        .filter(|appender| appender["encoder"]["kind"].as_str() == Some("palette"))
        .map(|appender| &appender["encoder"]);
    encoders.flat_map(|encoder| {
        let names = match encoder["palette"].as_mapping() {
            Some(palette) => palette.keys().filter_map(|key| key.as_str())
                .map(str::to_ascii_lowercase).collect(),
            None => palette::PaletteEncoderConfig::default_names().collect::<BTreeSet<_>>(),
        };
        references.iter().filter(move |name| !names.contains(*name)).cloned()
    }).collect()
}

fn apply_logger_placeholders(yaml: &str, saved_dir_suffix: Option<&str>) -> String {
    let suffix = saved_dir_suffix
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
        .replace(CHAT_LOG_PATH_PLACEHOLDER, &yaml_escape_double_quoted(&log_dir.join("chat.log").to_string_lossy()))
}

fn yaml_escape_double_quoted(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::test_resources as fixtures;
    use log::{Level, Log, Metadata, Record};
    use log4rs::encode::{Encode, EncoderConfig, writer::simple::SimpleWriter};

    #[test]
    fn logger_yaml_precedence_and_read_errors() {
        struct TempDir(PathBuf);
        impl Drop for TempDir {
            fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
        }
        let dir = TempDir(std::env::temp_dir().join(format!("unchained-logger-source-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())));
        std::fs::create_dir(&dir.0).unwrap();
        let check = |override_path: Option<&str>, source: &str, yaml: Option<&str>| {
            // Each child has its own CWD; do not change it for parallel tests.
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args(["--ignored", "--exact", "--nocapture",
                &format!("{}::logger_yaml_source_child", module_path!().split_once("::").unwrap().1)])
                .current_dir(&dir.0)
                .env_remove("UNCHAINED_TEST_LOGGER_OVERRIDE")
                .env("UNCHAINED_TEST_LOGGER_EXPECTED", serde_json::to_string(&(source, yaml)).unwrap());
            if let Some(path) = override_path { command.env("UNCHAINED_TEST_LOGGER_OVERRIDE", path); }
            let output = command.output().unwrap();
            assert!(output.status.success(), "{}\n{}",
                String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        };
        check(None, EMBEDDED_LOG4RS_DEFAULT_NAME, Some(EMBEDDED_LOG4RS_YAML));
        std::fs::write(dir.0.join("explicit.yaml"), "root: { level: warn }").unwrap();
        check(Some("explicit.yaml"), "explicit.yaml", Some("root: { level: warn }"));
        std::fs::write(dir.0.join("log4rs.yaml"), "root: { level: info }").unwrap();
        check(None, "log4rs.yaml", Some("root: { level: info }"));
        check(Some("explicit.yaml"), "explicit.yaml", Some("root: { level: warn }"));
        check(Some("missing.yaml"), "missing.yaml", None);
        std::fs::write(dir.0.join("log4rs.yaml"), [0xff]).unwrap();
        check(None, "log4rs.yaml", None);
        check(Some("explicit.yaml"), "explicit.yaml", Some("root: { level: warn }"));
        std::fs::remove_file(dir.0.join("log4rs.yaml")).unwrap();
        std::fs::create_dir(dir.0.join("log4rs.yaml")).unwrap();
        check(None, "log4rs.yaml", None);
    }

    #[test]
    #[ignore = "subprocess fixture for logger YAML source selection"]
    fn logger_yaml_source_child() {
        let override_path = std::env::var("UNCHAINED_TEST_LOGGER_OVERRIDE").ok();
        let expected = std::env::var("UNCHAINED_TEST_LOGGER_EXPECTED").unwrap();
        let (source, yaml): (String, Option<String>) = serde_json::from_str(&expected).unwrap();
        let loaded = load_logger_yaml(override_path.as_deref());
        match yaml {
            Some(yaml) => assert_eq!(loaded.unwrap(), (source.as_str(), yaml)),
            None => assert_eq!(loaded.unwrap_err().to_string(), format!("Failed to read logger config file: {source}")),
        }
    }

    #[test]
    fn invalid_logger_config_reports_pauses_and_resumes_with_defaults() {
        struct TempDir(PathBuf);
        impl Drop for TempDir {
            fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
        }
        let dir = TempDir(std::env::temp_dir().join(format!("unchained-logger-recovery-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())));
        std::fs::create_dir(&dir.0).unwrap();
        for (name, contents, override_path, error) in [
            ("read", &b"\xff"[..], None, "Failed to read logger config file"),
            ("yaml", &b"["[..], None, "Failed to parse logger config"),
            ("schema", &b"root: { level: invalid }"[..], None, "Failed to parse logger config"),
            ("appender", &b"appenders: { bad: { kind: missing } }\nroot: { level: info, appenders: [bad] }"[..], None, "Invalid logger config"),
            ("explicit", &b"root: { level: off }"[..], Some("missing.yaml"), "Failed to read logger config file"),
        ] {
            let cwd = dir.0.join(name);
            std::fs::create_dir(&cwd).unwrap();
            std::fs::write(cwd.join("log4rs.yaml"), contents).unwrap();
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args(["--ignored", "--exact", "--nocapture",
                &format!("{}::logger_config_recovery_child", module_path!().split_once("::").unwrap().1)])
                .current_dir(&cwd).env("LOCALAPPDATA", &cwd).env("NO_COLOR", "1")
                .env_remove("UNCHAINED_TEST_LOGGER_OVERRIDE");
            if let Some(path) = override_path { command.env("UNCHAINED_TEST_LOGGER_OVERRIDE", path); }
            let start = std::time::Instant::now();
            let output = command.output().unwrap();
            assert!(output.status.success(), "{name}: {}\n{}",
                String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            assert!(start.elapsed() >= std::time::Duration::from_secs(5), "{name}: skipped pause");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains(error), "{name}: {stderr}");
            assert!(stderr.contains(override_path.unwrap_or("log4rs.yaml")), "{name}: {stderr}");
            assert!(stderr.contains("in 5 seconds"), "{name}: {stderr}");
            let log = std::fs::read_to_string(cwd.join(r"Chivalry 2\Saved").join("Logs/unchained.log")).unwrap();
            assert!(log.contains("Rejected logging configuration:"), "{name}: {log}");
            assert!(log.contains("Logging continued after fallback"), "{name}: {log}");
        }
    }

    #[test]
    #[ignore = "subprocess fixture for logger configuration recovery"]
    fn logger_config_recovery_child() {
        let override_path = std::env::var("UNCHAINED_TEST_LOGGER_OVERRIDE").ok();
        init_with_path(override_path.as_deref(), None).unwrap();
        assert!(log::logger().enabled(&Metadata::builder().target("UnchainedPlugin").level(Level::Debug).build()));
        assert!(!log::logger().enabled(&Metadata::builder().target("UnchainedPlugin").level(Level::Trace).build()));
        sinfo!("Logging continued after fallback");
        log::logger().flush();
    }

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
        assert_eq!(records.len(), 3);
        assert_eq!(records[0]["message"], "ordinary");
        assert!(records[0]["attributes"].get("inline_colors").is_none());
        assert_eq!(records[1]["message"], "beforeafterend");
        let colors: serde_json::Value = serde_json::from_str(records[1]["attributes"]["inline_colors"].as_str().unwrap()).unwrap();
        assert_eq!(colors, serde_json::json!([
            {"utf8_offset": 6, "palette": "accent"}, {"utf8_offset": 11, "palette": null}
        ]));
        assert_eq!(records[2]["message"], "first line\r\nsecond 7\r\n");
        assert!(!file.contains("\\u001b"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "subprocess fixture for inline palette messages"]
    fn inline_palette_child() {
        let path = std::env::var("UNCHAINED_TEST_INLINE_FILE").unwrap();
        let mut yaml = fixtures::config("inline-palette");
        yaml["appenders"]["file"]["path"] = path.into();
        log4rs::init_config(config_from_value(yaml, "inline-palette").unwrap()).unwrap();
        sinfo!("ordinary");
        sinfo!(("before"), palette("accent"), "after", palette(), "end");
        sinfo!(palette("first"), "first line\r\n", palette("second"), ("second {}", 7), "\r\n");
        log::logger().flush();
    }

    fn render_encoder(registry: &Deserializers, yaml: serde_yaml::Value, record: &Record) -> Vec<u8> {
        let config: EncoderConfig = serde_yaml::from_value(yaml.clone()).unwrap();
        let encoder: Box<dyn Encode> = registry.deserialize(&config.kind, config.config)
            .unwrap_or_else(|error| panic!("{yaml:?}: {error}"));
        let mut output = Vec::new();
        encoder.encode(&mut SimpleWriter(&mut output), record).unwrap();
        output
    }

    fn console_config(encoder: serde_yaml::Value) -> anyhow::Result<Config> {
        let mut yaml = fixtures::config("console");
        yaml["appenders"]["console"]["encoder"] = encoder;
        config_from_value(yaml, "console-test")
    }

    #[test]
    fn embedded_configuration_filters() {
        let mut yaml: serde_yaml::Value = serde_yaml::from_str(EMBEDDED_LOG4RS_YAML).unwrap();
        // Exercise the shipped console config without opening log files.
        yaml["appenders"].as_mapping_mut().unwrap()
            .retain(|name, _| name.as_str() == Some("console"));
        yaml["root"]["appenders"] = vec!["console"].into();
        yaml["loggers"].as_mapping_mut().unwrap()
            .retain(|name, _| !name.as_str().is_some_and(|name| name.starts_with("chat_log")));
        let config = config_from_value(yaml, "embedded").unwrap();
        let logger = log4rs::Logger::new(config);
        assert!(logger.enabled(&Metadata::builder()
            .target("UnchainedPlugin::logging::log4rs").level(Level::Debug).build()));
        assert!(!logger.enabled(&Metadata::builder()
            .target("UnchainedPlugin::logging::log4rs").level(Level::Trace).build()));
        assert!(!logger.enabled(&Metadata::builder()
            .target("serenity::client").level(Level::Info).build()));
    }

    #[test]
    fn registered_encoders_reject_invalid_removal_patterns() {
        for kind in ["pattern", "palette"] {
            assert!(console_config(serde_yaml::to_value(serde_json::json!({
                "kind": kind, "inline-removals": ["["]
            })).unwrap()).is_err());
        }
    }

    #[test]
    fn isolated_chat_still_reaches_shared_json_appenders_once() {
        let prefix = std::env::temp_dir().join(format!("unchained-chat-routing-{}-{}",
            std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        for already_shared in [false, true] {
            let normal = prefix.with_extension(format!("{already_shared}.normal"));
            let chat = prefix.with_extension(format!("{already_shared}.chat"));
            let json = prefix.with_extension(format!("{already_shared}.json"));
            let mut yaml = serde_yaml::to_value(serde_json::json!({
                "appenders": {
                    "normal": {"kind": "file", "path": normal, "encoder": {"kind": "pattern", "pattern": "{m}{n}"}},
                    "chat": {"kind": "file", "path": chat, "encoder": {"kind": "pattern", "pattern": "{m}{n}"}},
                    "system": {"kind": "file", "path": chat, "encoder": {"kind": "pattern", "pattern": "{m}{n}"}},
                    "structured": {"kind": "file", "path": json, "encoder": {"kind": "json"}}
                },
                "root": {"level": "info", "appenders": ["normal", "structured"]},
                "loggers": {
                    "chat_log": {"level": "info", "appenders": ["chat"], "additive": false},
                    "chat_log::system": {"level": "info", "appenders": ["system"], "additive": false},
                    "chat_log::system::sender": {"level": "info", "appenders": ["chat"], "additive": false}
                }
            })).unwrap();
            if already_shared {
                yaml["loggers"]["chat_log"]["appenders"].as_sequence_mut().unwrap().push("structured".into());
            }
            let logger = log4rs::Logger::new(config_from_value(yaml, "chat-routing").unwrap());
            sinfo!(logger: &logger, target: "ordinary", "ordinary");
            for _ in 0..2 {
                sinfo!(logger: &logger, target: "chat_log::team", "hello");
            }
            sinfo!(logger: &logger, target: "chat_log::system", "notice");
            sinfo!(logger: &logger, target: "chat_log::system::sender", "console");
            logger.flush();
            assert_eq!(std::fs::read_to_string(&normal).unwrap().lines().collect::<Vec<_>>(), ["ordinary"]);
            assert_eq!(std::fs::read_to_string(&chat).unwrap().lines().collect::<Vec<_>>(), ["hello", "hello", "notice", "console"]);
            let records: Vec<serde_json::Value> = std::fs::read_to_string(&json).unwrap().lines()
                .map(|line| serde_json::from_str(line).unwrap()).collect();
            assert_eq!(records.iter().map(|record| (record["target"].as_str().unwrap(), record["message"].as_str().unwrap()))
                .collect::<Vec<_>>(), [("ordinary", "ordinary"), ("chat_log::team", "hello"),
                    ("chat_log::team", "hello"), ("chat_log::system", "notice"), ("chat_log::system::sender", "console")]);
            drop(logger);
            for path in [&normal, &chat, &json] { std::fs::remove_file(path).unwrap(); }
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
            let mut yaml = fixtures::encoder("boundary-escapes");
            yaml["pattern"] = pattern.into();
            let output = render_encoder(&deserializers, yaml, &Record::builder()
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
            let mut yaml = fixtures::encoder("module-palette");
            yaml["pattern"] = pattern.as_str().into();
            let output = render_encoder(&deserializers, yaml, &record);
            assert_eq!(output, expected, "{pattern}");
        }
        console_config(fixtures::encoder("native-fill")).unwrap();
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
            let mut yaml = fixtures::encoder("module-palette");
            yaml["pattern"] = pattern.as_str().into();
            yaml["palette"] = serde_yaml::to_value(palette).unwrap();
            let output = render_encoder(&deserializers, yaml,
                &Record::builder().level(Level::Info).args(format_args!("message")).build());
            assert_eq!(output, b"message}", "{pattern}");
        }
    }

    #[test]
    fn invalid_palette_configuration_is_rejected() {
        for options in fixtures::encoder("invalid").as_sequence().unwrap() {
            let mut yaml = options.clone();
            yaml["kind"] = "palette".into();
            assert!(console_config(yaml).is_err(), "{options:?}");
        }
    }

    #[test]
    fn configuration_errors_preserve_source_and_validation_boundaries() {
        let mut yaml = fixtures::config("console");
        yaml["root"]["level"] = "invalid-level".into();
        let error = config_from_value(yaml, "invalid-level-fixture").unwrap_err();
        assert_eq!(error.to_string(), "Failed to parse logger config invalid-level-fixture");
        assert!(error.chain().skip(1).any(|cause| cause.to_string().contains("invalid-level")));

        let mut yaml = fixtures::config("console");
        yaml["appenders"]["console"]["kind"] = "missing-appender-kind".into();
        let error = config_from_value(yaml, "invalid-appender-fixture").unwrap_err();
        assert!(error.to_string().starts_with("Invalid logger config invalid-appender-fixture:"));
        assert!(error.to_string().contains("missing-appender-kind"));

        let mut yaml = fixtures::config("console");
        yaml["root"]["appenders"] = vec!["missing-appender"].into();
        let error = config_from_value(yaml, "invalid-root-fixture").unwrap_err();
        assert_eq!(error.to_string(), "Invalid logger config invalid-root-fixture");
        assert!(error.chain().skip(1).any(|cause| cause.to_string().contains("missing-appender")));
    }

    #[test]
    fn file_appenders_support_registered_pattern_aliases() {
        struct TempDir(PathBuf);
        impl Drop for TempDir {
            fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
        }
        let dir = TempDir(std::env::temp_dir().join(format!("unchained-file-pattern-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())));
        std::fs::create_dir(&dir.0).unwrap();

        let fields = [("enclosing_function", "file_fixture")];
        let record = Record::builder().key_values(&fields).args(format_args!("message")).build();
        for appender_kind in ["file", "rolling_file"] {
            for encoder_kind in [None, Some("pattern")] {
                let path = dir.0.join("output.log");
                let mut yaml = fixtures::config("file-pattern");
                let file = &mut yaml["appenders"]["file"];
                file["kind"] = appender_kind.into();
                file["path"] = path.to_string_lossy().as_ref().into();
                file["encoder"]["pattern"] = "{fn}|{m}{n}".into();
                if let Some(kind) = encoder_kind { file["encoder"]["kind"] = kind.into(); }
                if appender_kind == "rolling_file" { file["policy"] = fixtures::config("rolling-policy"); }
                let config = config_from_value(yaml, "file-pattern-test").unwrap();
                let logger = log4rs::Logger::new(config);
                logger.log(&record);
                logger.flush();
                assert_eq!(std::fs::read_to_string(&path).unwrap().lines().collect::<Vec<_>>(),
                    ["file_fixture|message"], "{appender_kind}, {encoder_kind:?}");
            }
        }
    }

    #[test]
    fn console_adapter_preserves_builtin_configuration_validation() {
        for options in fixtures::appender("valid").as_sequence().unwrap() {
            let mut yaml = fixtures::config("console");
            yaml["appenders"]["console"] = options.clone();
            config_from_value(yaml, "console-options").unwrap();
        }
        for options in fixtures::appender("invalid").as_sequence().unwrap() {
            let mut yaml = fixtures::config("console");
            yaml["appenders"]["console"] = options.clone();
            let error = config_from_value(yaml, "invalid-console-options").unwrap_err();
            assert!(error.to_string().contains("invalid-console-options"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn inherit_only_palettes_preserve_channels_at_real_console_boundary() {
        let mut failures = Vec::new();
        for target in ["stdout", "stderr"] {
            for (fixture, update) in [
                ("inherit_foreground", "\x1b[49m"),
                ("inherit_background", "\x1b[39m"),
                ("inherit_both", ""),
                ("inherit_reset", "\x1b[0m"),
            ] {
                for unused_rgb in [false, true] {
                    let output = std::process::Command::new(std::env::current_exe().unwrap())
                        .args(["--ignored", "--exact", "--nocapture",
                            &format!("{}::inherit_console_child", module_path!().split_once("::").unwrap().1)])
                        .env("UNCHAINED_TEST_INHERIT_FIXTURE", fixture)
                        .env("UNCHAINED_TEST_CONSOLE_TARGET", target)
                        .env("UNCHAINED_TEST_UNUSED_RGB", unused_rgb.to_string())
                        .env("CLICOLOR_FORCE", "1").env_remove("NO_COLOR").env_remove("CLICOLOR")
                        .output().unwrap();
                    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
                    let bytes = if target == "stdout" { &output.stdout } else { &output.stderr };
                    let expected = format!("inherit-boundary:\x1b[38;2;18;52;86;48;2;171;205;239mA{update}B\x1b[0mC");
                    let text = String::from_utf8_lossy(bytes);
                    if !text.contains(&expected) {
                        failures.push(format!("{target}/{fixture}/unused_rgb={unused_rgb}: {text:?}"));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "subprocess fixture for inherit-only console palettes"]
    fn inherit_console_child() {
        let fixture = std::env::var("UNCHAINED_TEST_INHERIT_FIXTURE").unwrap();
        let mut appender = fixtures::appender(&fixture);
        appender["target"] = std::env::var("UNCHAINED_TEST_CONSOLE_TARGET").unwrap().into();
        if std::env::var("UNCHAINED_TEST_UNUSED_RGB").unwrap() == "true" {
            appender["encoder"]["palette"]["unused"] = "#112233".into();
        }
        let mut yaml = fixtures::config("console");
        yaml["appenders"]["console"] = appender;
        let logger = log4rs::Logger::new(config_from_value(yaml, "inherit-console-child").unwrap());
        logger.log(&Record::builder().args(format_args!(
            "inherit-boundary:\x1b[38;2;18;52;86;48;2;171;205;239mA")).build());
        logger.flush();
    }
}
