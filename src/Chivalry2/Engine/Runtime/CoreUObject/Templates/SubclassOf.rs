//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/Templates/SubclassOf.h>.

use std::ffi::c_void;

// Template specializations, not pointers to the wrapped UClass/IHttpRequest.
pub type TSubclassOfULoadoutSelection = c_void; // TSubclassOf<ULoadoutSelection>

pub type TSubclassOfAInventoryItem = c_void; // TSubclassOf<AInventoryItem>

pub type TSubclassOfATBLCharacter = c_void; // TSubclassOf<ATBLCharacter>
