mod image;
pub use image::{image, BASE, TEXT};
use std::sync::OnceLock;

#[unchained_macros_core::resolver_selection_key(require = [First, Second], fallback = Shared)]
#[derive(Debug)]
pub enum Flavor {
    First,
    Second,
    Shared,
}

static FLAVOR: OnceLock<Flavor> = OnceLock::new();

pub fn current_flavor() -> Flavor {
    *FLAVOR.get().expect("fixture selection not initialized")
}

impl unchained_macros_core::ResolverSelector for Flavor {
    fn current() -> Self {
        current_flavor()
    }
}

// Each exact child test gets its own immutable selection.
pub fn initialize_selection(flavor: Flavor) -> bool {
    if std::env::var_os("UNCHAINED_SELECTION_FIXTURE").is_some() {
        FLAVOR.set(flavor).unwrap();
        return true;
    }

    let thread = std::thread::current();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", thread.name().unwrap(), "--nocapture"])
        .env("UNCHAINED_SELECTION_FIXTURE", "1")
        .status()
        .unwrap();
    assert!(status.success());
    false
}
