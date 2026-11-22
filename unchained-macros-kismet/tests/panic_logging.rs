//! A user logger must not turn an isolated callback panic into native unwinding.
use std::{ffi::c_void, sync::atomic::{AtomicUsize, Ordering}};
use unchained_macros_kismet::observation::{self, Inputs};

struct PanickingLogger;
struct PanickingPayload;
impl Drop for PanickingPayload {
    fn drop(&mut self) { panic!("panic payload destructor fixture"); }
}
impl log::Log for PanickingLogger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool { true }
    fn log(&self, _: &log::Record<'_>) { std::panic::panic_any(PanickingPayload); }
    fn flush(&self) {}
}
static LOGGER: PanickingLogger = PanickingLogger;
static ORIGINALS: AtomicUsize = AtomicUsize::new(0);

unsafe fn entry(_: Inputs<'_>) { std::panic::panic_any(PanickingPayload); }

#[test]
fn panicking_entry_and_logger_keep_original_execution_and_scope() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Error);
    unsafe {
        observation::observe_exec(
            std::ptr::dangling_mut::<c_void>(), &[],
            Some(entry),
            || { ORIGINALS.fetch_add(1, Ordering::SeqCst); },
        );
        // The previous scope must have been restored despite callback/logger panics.
        observation::observe_exec(
            std::ptr::dangling_mut::<c_void>(), &[],
            None,
            || { ORIGINALS.fetch_add(1, Ordering::SeqCst); },
        );
    }
    assert_eq!(ORIGINALS.load(Ordering::SeqCst), 2);
}
