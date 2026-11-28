use std::sync::atomic::{AtomicUsize, Ordering};
use unchained_macros_core::isolate_observer;

struct PanickingLogger;

impl log::Log for PanickingLogger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, _: &log::Record<'_>) {
        panic!("logger failure");
    }

    fn flush(&self) {}
}

static LOGGER: PanickingLogger = PanickingLogger;

static DROPS: AtomicUsize = AtomicUsize::new(0);

struct Payload(bool);

impl Drop for Payload {
    fn drop(&mut self) {
        DROPS.fetch_add(1, Ordering::SeqCst);

        if self.0 {
            panic!("panic payload destructor failed");
        }
    }
}

#[test]

fn panic_isolation_does_not_call_a_throwing_logger_or_escape_payload_destructors() {
    log::set_logger(&LOGGER).unwrap();

    log::set_max_level(log::LevelFilter::Trace);

    assert!(isolate_observer(|| log::error!("observer logs through a throwing logger")).is_none());

    assert!(isolate_observer(|| panic!("observer failed")).is_none());

    assert!(isolate_observer(|| std::panic::panic_any(Payload(false))).is_none());

    assert!(isolate_observer(|| std::panic::panic_any(Payload(true))).is_none());

    assert_eq!(DROPS.load(Ordering::SeqCst), 2);
}
