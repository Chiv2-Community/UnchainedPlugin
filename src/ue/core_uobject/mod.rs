//! UE 4.25-plus: [CoreUObject](https://github.com/EpicGames/UnrealEngine/tree/4.25-plus/Engine/Source/Runtime/CoreUObject).

pub mod uclass;
pub mod uobject_globals;

pub use uclass::UClass;
pub use uobject_globals::{static_find_object, static_load_object};
