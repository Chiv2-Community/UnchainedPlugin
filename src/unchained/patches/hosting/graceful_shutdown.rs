//! Unregister the server before engine shutdown.

#[cfg(feature = "server_registration")]
use crate::chivalry2::tbl::tbl_engine::UTBLEngine;

#[cfg(feature = "server_registration")]
#[unchained_macros::bind_func_detour(UTBLEngine::PreExit)]
pub(crate) unsafe fn pre_exit_detour(this: *mut UTBLEngine) {
    let result = std::panic::catch_unwind(|| {
        crate::features::tokio_runtime::TOKIO_RUNTIME
            .block_on(crate::features::server_browser::registration::shutdown());
    });
    if result.is_err() {
        swarn!("Server-browser shutdown failed; continuing engine teardown");
    }
    unsafe {
        this.PreExit();
    }
}
