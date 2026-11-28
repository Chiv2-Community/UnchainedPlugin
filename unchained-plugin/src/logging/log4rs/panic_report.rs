//! Never invoke arbitrary logger code while Rust is running a panic hook.

use std::{
    backtrace::Backtrace,
    cell::Cell,
    io::{self, Write},
    panic,
    sync::{mpsc::{self, SyncSender}, OnceLock},
};
#[cfg(test)]
use std::thread::JoinHandle;

thread_local! {
    static FORWARDING: Cell<bool> = const { Cell::new(false) };
}

struct Reporter {
    sender: SyncSender<String>,
    // Retain the handle so subprocess tests can wait for failure containment.
    #[cfg(test)]
    worker: JoinHandle<()>,
}

static REPORTER: OnceLock<Option<Reporter>> = OnceLock::new();

fn stderr(message: &str) {
    // Unlike eprintln!, an unavailable/broken stderr must not panic.
    let _ = writeln!(io::stderr().lock(), "{message}");
}

fn reporter(target: &'static str) -> Option<&'static Reporter> {
    REPORTER.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel::<String>(16);
        let worker = std::thread::Builder::new().name("unchained-panic-log".into()).spawn(move || {
            FORWARDING.with(|active| active.set(true));
            for message in receiver {
                if let Err(payload) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    serror!(target: target, "{message}");
                    log::logger().flush();
                })) {
                    // The hook already wrote this logger panic to stderr. Do not
                    // run an arbitrary payload destructor or retry a broken logger.
                    std::mem::forget(payload);
                    stderr("Panic logger failed; subsequent panic reports use stderr only.");
                    break;
                }
            }
        });
        match worker {
            Ok(worker) => {
                #[cfg(not(test))]
                drop(worker);
                Some(Reporter { sender, #[cfg(test)] worker })
            }
            Err(error) => {
                stderr(&format!("Cannot start panic logger; using stderr only: {error}"));
                None
            }
        }
    }).as_ref()
}

pub(super) fn setup(target: &'static str) {
    let sender = reporter(target).map(|reporter| reporter.sender.clone());
    panic::set_hook(Box::new(move |info| {
        let message = info.payload().downcast_ref::<&str>().copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("Box<dyn Any>");
        let location = info.location().map(|location| location.to_string())
            .unwrap_or_else(|| "unknown location".into());
        let report = format!("PANIC at {location}: {message}\nStack Backtrace:\n{}",
            Backtrace::force_capture());
        stderr(&report);
        // A logger panic runs this hook on the forwarding thread, outside the
        // original hook. Never send that panic back to the same logger.
        if !FORWARDING.try_with(Cell::get).unwrap_or(true) {
            if let Some(sender) = &sender {
                // Bounded and nonblocking: hung loggers cannot hold up unwinding.
                // Full/disconnected queues need no further fallback: stderr won.
                let _ = sender.try_send(report);
            }
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::{AtomicBool, Ordering}, Mutex};

    #[test]
    fn actual_plugin_panic_hook_contains_throwing_logger_and_preserves_reports() {
        for mode in ["healthy", "throwing"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", "--nocapture",
                    &format!("{}::panic_hook_child", module_path!().split_once("::").unwrap().1)])
                .env("UNCHAINED_TEST_PANIC_LOGGER", mode).output().unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "{mode}: {stderr}");
            assert!(stderr.contains("isolated observer panic"), "{mode}: {stderr}");
            assert!(stderr.contains("Stack Backtrace:"), "{mode}: {stderr}");
            if mode == "throwing" {
                assert!(stderr.contains("throwing custom logger"), "{stderr}");
                assert!(stderr.contains("subsequent panic reports use stderr only"), "{stderr}");
                assert!(stderr.contains("second observer panic"), "{stderr}");
            }
        }
    }

    #[test]
    #[ignore = "subprocess fixture for the actual plugin panic hook"]
    fn panic_hook_child() {
        struct Logger {
            throwing: bool,
            reports: Mutex<Vec<String>>,
            delivered: std::sync::Condvar,
        }
        impl log::Log for Logger {
            fn enabled(&self, _: &log::Metadata<'_>) -> bool { true }
            fn log(&self, record: &log::Record<'_>) {
                if self.throwing { panic!("throwing custom logger"); }
                self.reports.lock().unwrap().push(record.args().to_string());
                self.delivered.notify_one();
            }
            fn flush(&self) {}
        }
        let logger = Box::leak(Box::new(Logger {
            throwing: std::env::var("UNCHAINED_TEST_PANIC_LOGGER").unwrap() == "throwing",
            reports: Mutex::new(Vec::new()), delivered: std::sync::Condvar::new(),
        }));
        log::set_logger(logger).unwrap();
        log::set_max_level(log::LevelFilter::Error);
        // Install the public hook, not a test substitute/default Rust hook.
        crate::logging::log4rs::setup_panic_logger();
        static CAUGHT: AtomicBool = AtomicBool::new(false);
        extern "system" fn observer_boundary() {
            CAUGHT.store(panic::catch_unwind(|| panic!("isolated observer panic")).is_err(),
                Ordering::Relaxed);
        }
        observer_boundary();
        assert!(CAUGHT.load(Ordering::Relaxed));
        if logger.throwing {
            let worker = &REPORTER.get().unwrap().as_ref().unwrap().worker;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !worker.is_finished() {
                assert!(std::time::Instant::now() < deadline, "panic reporter did not stop");
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert!(panic::catch_unwind(|| panic!("second observer panic")).is_err());
        } else {
            let (reports, timeout) = logger.delivered.wait_timeout_while(
                logger.reports.lock().unwrap(), std::time::Duration::from_secs(5),
                |reports| reports.is_empty()).unwrap();
            assert!(!timeout.timed_out(), "panic report did not reach healthy logger");
            assert_eq!(reports.len(), 1);
            assert!(reports[0].contains("isolated observer panic"));
            assert!(reports[0].contains("Stack Backtrace:"));
        }
    }
}
