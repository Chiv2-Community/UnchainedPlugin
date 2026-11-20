//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Online/ICMP/Public/Icmp.h>.

use crate::Chivalry2::Engine::FString;

use std::ffi::c_void;

define_pattern_resolver!(
    UDPEcho,
    ["48 89 5C 24 20 55 56 57 48 81 EC 90 00 00 00 49 8B D8 0F 29 B4 24 80 00 00 00 48 8B E9"]
);

/// Static native ping helpers; no instance layout is assumed.
pub struct FUDPPing {
    _private: (),
}

#[unchained_macros::runtime_bindings]
impl FUDPPing {
    /// # Safety
    /// Arguments must satisfy the native UDPEcho ABI and callback lifetime.
    #[unchained_macros::bind_resolved_func(UDPEcho)]
    pub unsafe fn UDPEcho(
        endpoint: *mut FString,
        timeout: f32,
        callback: *mut TFunctionFIcmpEchoResult,
    ) {
    }
}

/// Opaque `TFunction<void(FIcmpEchoResult)>`; passed only through native pointers.
pub type TFunctionFIcmpEchoResult = c_void;
