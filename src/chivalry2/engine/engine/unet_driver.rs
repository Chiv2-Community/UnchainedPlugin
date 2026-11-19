//! UE 4.25-plus: [UNetDriver](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/NetDriver.h).
//!
//! Native net-driver mode patch.
use crate::tools::hook_globals::cli_args;
use crate::chivalry2::engine::UObject;

define_pattern_resolver!(UNetDriver_GetNetMode, [
    "48 83 EC 28 48 8B 01 ?? ?? ?? ?? ?? ?? 84 C0 ?? ?? 33 C0 38 ?? ?? ?? ?? 02 0F 95 C0 FF C0 48 83 C4",
]);

/// Partial UNetDriver layout; fields after ServerConnection are omitted.
#[repr(C)]
pub struct UNetDriver {
    pub base: UObject,
    _padding_to_server_connection: [u8; 0x60],
    pub server_connection: *mut UObject, // 0x0088
}

CREATE_PATCH!(UNetDriver_GetNetMode, BYTES, &[0xB8, 0x01, 0x00, 0x00, 0x00, 0xC3], IF { cli_args().apply_desync_patch });
