//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/NetDriver.h>.

use crate::Chivalry2::Engine::UObject;

define_pattern_resolver!(UNetDriver__GetNetMode, [
    "48 83 EC 28 48 8B 01 ?? ?? ?? ?? ?? ?? 84 C0 ?? ?? 33 C0 38 ?? ?? ?? ?? 02 0F 95 C0 FF C0 48 83 C4",
]);

/// Partial UNetDriver layout; fields after ServerConnection are omitted.
#[unchained_macros_core::runtime_bindings]
#[repr(C)]
pub struct UNetDriver {
    pub base: UObject,
    _padding_to_server_connection: [u8; 0x60],
    pub server_connection: *mut UObject, // 0x0088
}
