//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Kismet/GameplayStatics.h>.

use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::TArray;
use crate::Chivalry2::Engine::UClass;
use crate::Chivalry2::Engine::UObject;
use crate::Chivalry2::Engine::USaveGame;

/// Static native helpers only; this is not a UObject instance layout.
#[unchained_macros::runtime_bindings]
pub struct UGameplayStatics {
    _private: (),
}

#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Engine.GameplayStatics")]
impl UGameplayStatics {
    /// # Safety
    /// Call on the engine thread with live context/class pointers and writable
    /// engine-compatible array storage. Entries are borrowed native actors.
    pub unsafe fn GetAllActorsOfClass(
        context: *const UObject,
        actor_class: *mut UClass,
        output: *mut TArray<*mut UObject>,
    ) {
    }

    pub(crate) unsafe fn LoadGameFromSlot<'a>(slot: &'a FString, index: i32) -> *mut USaveGame {}

    pub(crate) unsafe fn SaveGameToSlot<'a>(
        game: *mut USaveGame,
        slot: &'a FString,
        index: i32,
    ) -> bool {
    }
}
