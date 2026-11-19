//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/GameMode.h>.

use crate::chivalry2::engine::FName;

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
    #[bind_resolved_func_detour(SetMatchState)]
    pub(crate) unsafe fn set_match_state_detour(self: *mut Self, state: FName) {
        let name = state.to_string();
        sinfo!("Match state: {}", name);
        crate::features::motd::set_match_active(name == "InProgress");
        unsafe {
            self.set_match_state(state);
        }
    }
}
