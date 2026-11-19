//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Plugins/Online/OnlineSubsystemUtils/Source/OnlineSubsystemUtils/Classes/IpConnection.h>.

use crate::chivalry2::engine::*;

#[repr(C)]
pub struct UIpConnection {
    pub base: UObject,
    _padding_to_remote_addr: [u8; 0xF0],
    pub remote_addr: *const FInternetAddr, // 0x0118 (shared pointer)
}

unsafe impl crate::chivalry2::engine::UObjectType for UIpConnection {
    const CLASS_NAME: &'static str = "IpConnection";
}
