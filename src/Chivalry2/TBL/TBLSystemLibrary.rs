//! Native source: TBL/Source/TBL/TBLSystemLibrary.h (shipping PDB).

use crate::Chivalry2::Engine::UObject;
use crate::Chivalry2::TBL::ATBLGameMode;

define_pattern_resolver![GetTBLGameMode, {
    EGS : ["40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51"], // EGS
    OTHER : [
        "40 53 48 83 EC 20 48 8B D9 48 85 C9 74 60 48 8B 01 FF 90 ?? ?? ?? ?? 48 85 C0 75 23 0F 1F 40 00 48 8B 5B 20 48 85 DB 74 11 48 8B 03 48 8B CB FF 90 ?? ?? ?? ?? 48 85 C0 74 E6 48 85 C0 74 2F 48 8B 98 28",
        "40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51" // EGS 2.11.4
        ]
}];

/// Static native helpers only; this is not a UObject instance layout.
pub struct UTBLSystemLibrary {
    _private: (),
}

#[unchained_macros::runtime_bindings]
impl UTBLSystemLibrary {
    /// # Safety
    /// Context must be a live native object on the engine thread. The returned
    /// game mode is borrowed and can be null in non-game worlds.
    #[bind_resolved_func(self::GetTBLGameMode)]
    pub unsafe fn GetTBLGameMode(context: *mut UObject) -> *mut ATBLGameMode {}
}
