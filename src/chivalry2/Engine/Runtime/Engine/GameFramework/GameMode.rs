//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/GameMode.h>.

use crate::chivalry2::Engine::FName;

define_pattern_resolver!(
    SetMatchState,
    ["48 89 5C 24 ?? 56 48 83 EC 20 48 8B DA 48 8B F1 48 39 91 ?? ?? ?? ??"]
);

/// Opaque engine-owned game mode; no Rust layout is assumed.
#[repr(C)]
pub struct AGameMode {
    _opaque: [u8; 0],
}

#[unchained_macros::runtime_bindings]
impl AGameMode {
    #[unchained_macros::bind_resolved_func(SetMatchState)]
    pub(crate) unsafe fn SetMatchState(self: *mut Self, state: FName) {}
}
