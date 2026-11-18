//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/NetDriver.h>.

#![allow(non_snake_case)]
use crate::engine::*;

/// Partial UNetDriver layout; fields after ServerConnection are omitted.
#[repr(C)]
pub struct UNetDriver {
    pub base: UObject,
    _padding_to_server_connection: [u8; 0x60],
    pub server_connection: *mut UObject, // 0x0088
}
