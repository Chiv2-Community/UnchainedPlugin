//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/UObjectBase.h>.

use crate::chivalry2::Engine::*;

use crate::chivalry2::Engine::UClass;

#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct UObjectBase {
    #[vtable()]
    vtable: *const (),
    pub object_flags: EObjectFlags,
    pub internal_index: i32,
    pub class_private: *const UClass,
    pub name_private: FName,
    pub outer_private: *const UObject,
}
