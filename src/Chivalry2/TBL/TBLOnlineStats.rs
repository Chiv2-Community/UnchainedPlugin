//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLOnlineStats.h>.
//! Native owner: TBL/Source/TBL/Online/TBLOnlineStats.h/.cpp.

define_pattern_resolver!(FTBLOnlineStats__SetStatInner, [
    "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 48 83 EC 30 48 8B F9 48 63 F2 48 8D 4C 24 20 41 8B E8"
]);

/// Opaque native statistics object. Do not construct in Rust.
#[unchained_macros::runtime_bindings]
#[repr(C)]
pub struct FTBLOnlineStats {
    _opaque: [u8; 0],
}

#[unchained_macros::runtime_bindings]
impl FTBLOnlineStats {
    // Preserve the existing ABI and suppression of the native call.
    #[unchained_macros::bind_resolved_func(FTBLOnlineStats__SetStatInner)]
    pub(crate) unsafe fn SetStatInner(self: *mut Self, _p1: i32, _p2: i32) {}
}
