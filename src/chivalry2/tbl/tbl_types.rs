//! Native source: TBL/Source/TBL/TBLTypes.cpp, namespace UGameplay (shipping PDB).

use crate::chivalry2::engine::UObject;

define_pattern_resolver!(UGameplay_IsDedicatedServer, [
    "48 83 EC 28 48 85 C9 ?? ?? BA 01 00 00 00 ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B C8 ?? ?? ?? ?? ?? 83 F8 01 0F 94 C0 48",
]);

/// Rust adapter for the native UGameplay namespace, not a UObject class.
pub struct UGameplay {
    _private: (),
}

#[unchained_macros::runtime_bindings]
impl UGameplay {
    #[unchained_macros::bind_resolved_func(UGameplay_IsDedicatedServer)]
    pub(crate) unsafe fn is_dedicated_server(context: *mut UObject) -> bool {}
}
