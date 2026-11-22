//! A user logger must not turn an isolated callback panic into native unwinding.
use std::{ffi::c_void, sync::atomic::{AtomicUsize, Ordering}};
use unchained_macros_kismet::observation::{self, Callbacks, Inputs};

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
static RELEASES: AtomicUsize = AtomicUsize::new(0);

unsafe fn entry(_: Inputs<'_>) { std::panic::panic_any(PanickingPayload); }
unsafe fn finish(_: *mut (), _: *const ()) { panic!("exit fixture"); }
unsafe fn release(state: *mut ()) {
    unsafe { drop(Box::from_raw(state.cast::<u8>())); }
    RELEASES.fetch_add(1, Ordering::SeqCst);
}
unsafe fn snapshot(_: Inputs<'_>) -> Option<unchained_macros_core::ExitObservation> {
    Some(unsafe { unchained_macros_core::ExitObservation::new(
        Box::into_raw(Box::new(7u8)).cast(), finish, release,
    ) })
}

#[test]
fn panicking_entry_exit_and_logger_keep_original_snapshot_cleanup_and_scope() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Error);
    let mut result = 0u8;
    unsafe {
        observation::observe_exec(
            std::ptr::dangling_mut::<c_void>(), (&mut result as *mut u8).cast(), &[],
            Callbacks { entry: Some(entry), snapshot: Some(snapshot), ..Callbacks::default() },
            || { ORIGINALS.fetch_add(1, Ordering::SeqCst); },
        );
        // The previous scope must have been restored despite both callback panics.
        observation::observe_exec(
            std::ptr::dangling_mut::<c_void>(), std::ptr::null_mut(), &[],
            Callbacks::default(),
            || { ORIGINALS.fetch_add(1, Ordering::SeqCst); },
        );
    }
    assert_eq!(ORIGINALS.load(Ordering::SeqCst), 2);
    assert_eq!(RELEASES.load(Ordering::SeqCst), 1);
}
