//! ArgonSDK main: [UTBLEngine](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLEngine.h).

/// Engine-owned subclass. No native fields or Rust construction are exposed.
#[repr(C)]
pub struct UTBLEngine { _opaque: [u8; 0] }

#[cfg(feature = "server_registration")]
define_pattern_resolver!(UTBLEnginePreExit, [
    "40 55 53 41 54 41 55 48 8D 6C 24 A8 48 81 EC 58 01 00 00 4C 8B E9"
]);

#[unchained_macros::runtime_bindings]
impl UTBLEngine {
    #[cfg(feature = "server_registration")]
    #[bind_resolved_func_detour(UTBLEnginePreExit)]
    pub(crate) unsafe fn pre_exit_detour(self: *mut Self) {
        let result = std::panic::catch_unwind(|| {
            crate::features::tokio_runtime::TOKIO_RUNTIME
                .block_on(crate::features::server_browser::registration::shutdown());
        });
        if result.is_err() {
            swarn!("Server-browser shutdown failed; continuing engine teardown");
        }
        unsafe { self.pre_exit(); }
    }
}
