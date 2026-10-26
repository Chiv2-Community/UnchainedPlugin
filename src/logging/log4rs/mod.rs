use anyhow::Context;
use log4rs::config::{Config, Deserializers, RawConfig};
use console::{ConsoleAppenderDeserializer, ConsoleEncoderDeserializer};
use pattern::PatternEncoderDeserializer;

mod console;
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
    init_logger_from_yaml(&resolved_yaml, config_source_name)?;
    setup_panic_logger();

    Ok(())
}

fn load_logger_yaml(override_path: Option<&str>) -> anyhow::Result<String> {
    match override_path {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read logger config file: {}", path)),
        None => Ok(EMBEDDED_LOG4RS_YAML.to_string()),
    }
}

fn init_logger_from_yaml(yaml: &str, config_source_name: &str) -> anyhow::Result<()> {
    let config = config_from_yaml(yaml, config_source_name)?;
    log4rs::init_config(config).with_context(|| {
        format!("Failed to initialize logger from config {}", config_source_name)
    })?;
    Ok(())
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

/// Register the module-pattern extension and pastel console integration.
pub fn register(deserializers: &mut Deserializers) {
    deserializers.insert("module_console", ConsoleEncoderDeserializer);
    deserializers.insert("console", ConsoleAppenderDeserializer);
    deserializers.insert("pattern", PatternEncoderDeserializer);
}

fn apply_logger_placeholders(yaml: &str) -> String {
    let suffix = cli_args()
        .saved_dir_suffix
        .as_deref()
        .map(|value| format!("_{}", value))
        .unwrap_or_default();

    let log_dir = expand_env_path(&format!(r"%LOCALAPPDATA%\Chivalry 2\Saved{}", suffix))
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

fn expand_env_path(path: &str) -> Option<PathBuf> {
    if let Some(stripped) = path.strip_prefix("%LOCALAPPDATA%") {
        if let Ok(base) = std::env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(base).join(stripped.trim_start_matches(['\\', '/'])));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::{Level, Log, Metadata};

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
        let config = config_from_yaml(&format!(r#"
appenders:
  console:
    kind: console
    target: {target}
    encoder:
      kind: module_console
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
    fn embedded_console_config_loads() {
        let mut yaml: serde_yaml::Value = serde_yaml::from_str(EMBEDDED_LOG4RS_YAML).unwrap();
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
    fn default_file_formats_use_five_column_level_and_full_module() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(EMBEDDED_LOG4RS_YAML).unwrap();
        for name in ["file", "kismet"] {
            let pattern = yaml["appenders"][name]["encoder"]["pattern"].as_str().unwrap();
            assert!(pattern.contains("{l:5}"));
            assert!(pattern.contains("{M}"));
            assert!(!pattern.contains("{t}"));
        }
    }

    #[test]
    fn console_adapter_delegates_pattern_encoders() {
        config_from_yaml(r#"
appenders:
  console:
    kind: console
    encoder:
      pattern: "{l} [{t}] {m}{n}"
root:
  level: info
  appenders: [console]
"#, "override").unwrap();
    }

    #[test]
    fn registered_encoders_accept_short_and_full_module_fields() {
        use log4rs::encode::{Encode, EncoderConfig, writer::simple::SimpleWriter};

        let mut deserializers = Deserializers::default();
        register(&mut deserializers);
        for yaml in [
            "kind: pattern\npattern: '{sm}|{M}|{t}|{fn}'",
            "kind: module_console\nheader_pattern: '{sm}|'\nbody_pattern: '{M}|{t}|{fn}'",
        ] {
            let config: EncoderConfig = serde_yaml::from_str(yaml).unwrap();
            let encoder: Box<dyn Encode> = deserializers.deserialize(&config.kind, config.config).unwrap();
            let mut output = Vec::new();
            encoder.encode(&mut SimpleWriter(&mut output), &log::Record::builder()
                .module_path(Some("UnchainedPlugin::messages")).target("filter::target").build()).unwrap();
            assert_eq!(String::from_utf8(output).unwrap(), "UP::messages|UnchainedPlugin::messages|filter::target|???");
        }
    }

    #[test]
    fn console_adapter_preserves_builtin_configuration_validation() {
        for options in ["", "target: null", "target: stderr\n    tty_only: true"] {
            config_from_yaml(&format!("appenders:\n  console:\n    kind: console\n    {options}\nroot:\n  level: info\n  appenders: [console]\n"), "console-options").unwrap();
        }
        for options in ["target: invalid", "unknown: true", "encoder:\n      kind: module_console\n      unknown: true"] {
            assert!(config_from_yaml(&format!("appenders:\n  console:\n    kind: console\n    {options}\nroot:\n  level: info\n  appenders: [console]\n"), "invalid-console-options").is_err());
        }
    }

    #[test]
    fn invalid_encoder_is_not_silently_dropped() {
        let error = config_from_yaml(r#"
appenders:
  console:
    kind: console
    encoder:
      kind: missing_encoder
root:
  level: debug
  appenders: [console]
"#, "bad.yaml").unwrap_err();
        assert!(error.to_string().contains("bad.yaml"));
    }
}
