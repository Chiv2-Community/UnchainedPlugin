//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Kismet/GameplayStatics.h>.

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::TArray;
use crate::chivalry2::engine::UClass;
use crate::chivalry2::engine::UObject;
use crate::chivalry2::engine::USaveGame;

define_pattern_resolver!(GetAllActorsOfClass,["40 55 53 56 57 48 8D 6C 24 C1 48 81 EC A8 00 00 00 48 8B 05 70 B1 70 02 48 33 C4 48 89 45 1F 41"]);

define_pattern_resolver!(
    LoadGameFromSlot,
    ["48 8B C4 55 57 48 8D A8 E8 FE FF FF 48 81 EC 08 02 00 00 48 89 58 08 33 FF 48 89 70"]
);

define_pattern_resolver!(SaveGameToSlot,["48 89 5C 24 08 48 89 74 24 10 57 48 83 EC 40 48 8B DA 33 F6 48 8D 54 24 30 48 89 74 24 30 48 89 74 24 38 41"]);

/// Static native helpers only; this is not a UObject instance layout.
pub struct UGameplayStatics {
    _private: (),
}

#[unchained_macros::runtime_bindings]
impl UGameplayStatics {
    /// # Safety
    /// Call on the engine thread with live context/class pointers and writable
    /// engine-compatible array storage. Entries are borrowed native actors.
    #[bind_resolved_func(self::GetAllActorsOfClass)]
    pub unsafe fn GetAllActorsOfClass(
        context: *const UObject,
        actor_class: *mut UClass,
        output: *mut TArray<*mut UObject>,
    ) {
    }

    #[unchained_macros::bind_resolved_func(LoadGameFromSlot)]
    pub(crate) unsafe fn LoadGameFromSlot<'a>(slot: &'a FString, index: i32) -> *mut USaveGame {}

    #[unchained_macros::bind_resolved_func(SaveGameToSlot)]
    pub(crate) unsafe fn SaveGameToSlot<'a>(
        game: *mut USaveGame,
        slot: &'a FString,
        index: i32,
    ) -> bool {
    }
}
