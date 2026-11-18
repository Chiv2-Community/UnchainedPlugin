//! UE 4.25-plus: [CoreUObject](https://github.com/EpicGames/UnrealEngine/tree/4.25-plus/Engine/Source/Runtime/CoreUObject).

pub mod uclass;
pub mod fsoft_object_path;
pub mod fsoft_object_ptr;
pub use fsoft_object_path::FSoftObjectPath;
pub use fsoft_object_ptr::{FSoftObjectPtr, TSoftClassPtr};
pub mod uobject_globals;

pub use uclass::UClass;
pub use uobject_globals::{static_find_object, static_load_object, get_uobject_from_path};
