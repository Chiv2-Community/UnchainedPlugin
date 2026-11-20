//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Public/EngineUtils.h>.

use crate::Chivalry2::Engine::FName;
use crate::Chivalry2::Engine::TCHAR;

define_pattern_resolver!(FConsoleOutputDevice__Serialize,["48 89 5C 24 10 48 89 6C 24 18 48 89 74 24 20 41 56 48 83 EC 30 41 0F B6 E8 49 8B D9 44 0F B6 C5 48 8B"]);

#[repr(C)]
pub struct FConsoleOutputDevice {
    _opaque: [u8; 0],
}

#[unchained_macros::runtime_bindings]
impl FConsoleOutputDevice {
    #[unchained_macros::bind_resolved_func(FConsoleOutputDevice__Serialize)]
    pub(crate) unsafe fn Serialize(
        self: *mut Self,
        text: *const TCHAR,
        verbosity: u8,
        category: *const FName,
    ) {
    }
}
