//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Plugins/Online/OnlineSubsystemUtils/Source/OnlineSubsystemUtils/Classes/IpNetDriver.h>.

#![allow(non_snake_case)]
use crate::engine::*;

#[repr(C)]
pub struct UIpNetDriver {
    pub base: UNetDriver,
    _padding_to_local_addr: [u8; 0x620],
    pub local_addr: *const FInternetAddr, // 0x06B0 (shared pointer)
}

// SAFETY: These supported native layouts have their UObject prefix at offset zero.
unsafe impl crate::ue::UObjectType for UIpNetDriver {
    const CLASS_NAME: &'static str = "IpNetDriver";
}
