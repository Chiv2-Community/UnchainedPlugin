//! Native source: TBL/Source/TBL/TBLEngine.h.
//! SDK header: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLEngine.h>.

#[cfg(feature = "server_registration")]
define_pattern_resolver!(
    UTBLEngine__PreExit,
    ["40 55 53 41 54 41 55 48 8D 6C 24 A8 48 81 EC 58 01 00 00 4C 8B E9"]
);

/// Engine-owned subclass. No native fields or Rust construction are exposed.
#[repr(C)]
pub struct UTBLEngine {
    _opaque: [u8; 0],
}

#[unchained_macros::runtime_bindings]
impl UTBLEngine {
    #[cfg(feature = "server_registration")]
    #[unchained_macros::bind_resolved_func(UTBLEngine__PreExit)]
    pub(crate) unsafe fn PreExit(self: *mut Self) {}
}
