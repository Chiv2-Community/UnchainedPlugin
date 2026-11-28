//! Shared output for opt-in Blueprint print listeners.
//!
//! `blueprint_printstring_log` enables printers; `kismet_diagnostics` also enables
//! existing `kismet_log` diagnostics. Neither changes default/dev features.
//! The embedded logger routes `blueprint_log` to the console and `blueprint.log`.
//! Custom logger configs remain authoritative: add that target and appenders there,
//! using `"__BLUEPRINT_LOG_PATH__"` for the plugin's default file location.
//!
//! `blueprint_filter.txt` lives in `plugin_log_dir()`, even with custom appenders.
//! One trimmed, exact full-message string per line; blank lines and trimmed `#`
//! comments are ignored. Matching is case-sensitive, with no substring/regex matching
//! or trimming of emitted messages. Multiline messages cannot be listed.
//! Loaded on first use, including missing/unreadable files; edits require restart.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{atomic::{AtomicBool, Ordering}, OnceLock};

pub(crate) fn log_blueprint_print(kind: &str, message: &str) {
    if blueprint_spam_filter().is_some_and(|filter| filter.contains(message)) {
        return;
    }
    sinfo!(target: "blueprint_log", kind = kind; "{}", message);
}

fn parse_filter(text: &str) -> HashSet<String> {
    text.lines().map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned).collect()
}

fn load_filter(path: &Path) -> std::io::Result<HashSet<String>> {
    std::fs::read_to_string(path).map(|text| parse_filter(&text))
}

fn blueprint_spam_filter() -> Option<&'static HashSet<String>> {
    static FILTER: OnceLock<(std::path::PathBuf, std::io::Result<HashSet<String>>)> = OnceLock::new();
    static REPORTED: AtomicBool = AtomicBool::new(false);
    let (path, loaded) = FILTER.get_or_init(|| {
        let path = crate::logging::log4rs::plugin_log_dir().join("blueprint_filter.txt");
        let loaded = load_filter(&path);
        (path, loaded)
    });
    // Publish before logging: a custom logger may reenter a print listener.
    // Never log while initializing FILTER or recursively through blueprint_log.
    if !REPORTED.swap(true, Ordering::Relaxed) {
        match loaded {
            Ok(filter) => sinfo!("Loaded {} Blueprint filter entries from {}; edits require restart",
                filter.len(), path.display()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound =>
                sinfo!("No Blueprint filter at {}; no messages suppressed; restart after creating it",
                    path.display()),
            Err(error) => swarn!("Cannot read Blueprint filter {}: {}; no messages suppressed; edits require restart",
                path.display(), error),
        }
    }
    loaded.as_ref().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_ignores_blank_comments_and_duplicates() {
        let filter = parse_filter("\r\n # comment\r\n  hello  \r\nhello\n界\nfoo # literal\n");
        assert_eq!(filter, HashSet::from(["hello".into(), "界".into(), "foo # literal".into()]));
        for message in ["Hello", " hello", "hello ", "hello world", "hello\nworld", "# comment", ""] {
            assert!(!filter.contains(message), "{message:?}");
        }
    }

    #[test]
    fn filter_file_errors_and_once_only_loading() {
        struct TempDir(std::path::PathBuf);
        impl Drop for TempDir {
            fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
        }
        let dir = TempDir(std::env::temp_dir().join(format!("unchained-blueprint-filter-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())));
        std::fs::create_dir(&dir.0).unwrap();
        let path = dir.0.join("blueprint_filter.txt");
        let missing = OnceLock::new();
        assert_eq!(missing.get_or_init(|| load_filter(&path)).as_ref().unwrap_err().kind(),
            std::io::ErrorKind::NotFound);
        std::fs::write(&path, "first\n").unwrap();
        assert!(missing.get_or_init(|| panic!("missing file retried")).is_err());
        let loaded = OnceLock::new();
        assert!(loaded.get_or_init(|| load_filter(&path)).as_ref().unwrap().contains("first"));
        std::fs::write(&path, "second\n").unwrap();
        let filter = loaded.get_or_init(|| panic!("filter reloaded")).as_ref().unwrap();
        assert!(filter.contains("first"));
        assert!(!filter.contains("second"));
        std::fs::write(&path, [0xff]).unwrap();
        assert_eq!(load_filter(&path).unwrap_err().kind(), std::io::ErrorKind::InvalidData);
        assert!(load_filter(&dir.0).is_err());
    }
}
