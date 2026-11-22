//! Native owner: TBL/Source/TBL/TBLTypes.h/.cpp.
//! Native source unavailable: neither checkout contains this header or the UGameplay namespace.

use crate::Chivalry2::Engine::UObject;

define_pattern_resolver!(UGameplay__IsDedicatedServer, [
    "48 83 EC 28 48 85 C9 ?? ?? BA 01 00 00 00 ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B C8 ?? ?? ?? ?? ?? 83 F8 01 0F 94 C0 48",
]);

/// Rust adapter for the native UGameplay namespace, not a UObject class.
#[unchained_macros_core::runtime_bindings]
pub struct UGameplay {
    _private: (),
}

#[unchained_macros_core::runtime_bindings]
impl UGameplay {
    #[unchained_macros_core::bind_resolved_func(UGameplay__IsDedicatedServer)]
    pub(crate) unsafe fn IsDedicatedServer(context: *mut UObject) -> bool {}
}
