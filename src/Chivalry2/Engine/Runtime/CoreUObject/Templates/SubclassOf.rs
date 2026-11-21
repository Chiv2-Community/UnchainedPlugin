//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/Templates/SubclassOf.h>.

use std::ffi::c_void;

// Template specializations, not pointers to the wrapped UClass/IHttpRequest.
#[unchained_macros::runtime_bindings]
pub type TSubclassOfULoadoutSelection = c_void; // TSubclassOf<ULoadoutSelection>

#[unchained_macros::runtime_bindings]
pub type TSubclassOfAInventoryItem = c_void; // TSubclassOf<AInventoryItem>

#[unchained_macros::runtime_bindings]
pub type TSubclassOfATBLCharacter = c_void; // TSubclassOf<ATBLCharacter>
