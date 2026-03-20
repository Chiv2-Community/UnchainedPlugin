use anyhow::Context;
use log4rs::init_raw_config;
use log4rs::config::RawConfig;

#[allow(unused_imports)]
use super::syslog::SyslogAppender;
use std::backtrace::Backtrace;
use std::panic;
use log::error;
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

        error!(
            "PANIC at {}: {}\nStack Backtrace:\n{}",
            location, msg, backtrace
        );
    }));
}

pub fn init_syslog() -> anyhow::Result<()> {
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
    let raw_config: RawConfig = serde_yaml::from_str(yaml).with_context(|| {
        format!(
            "Failed to parse logger config {}",
            config_source_name
        )
    })?;
    init_raw_config(raw_config).with_context(|| {
        format!(
            "Failed to initialize logger from config {}",
            config_source_name
        )
    })?;
    Ok(())
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
            "Failed to create log directory {}: {}",
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
