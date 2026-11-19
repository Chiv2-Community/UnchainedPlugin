//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/Field.h>.

use crate::chivalry2::engine::*;

use std::ffi::c_void;

#[derive(Debug)]
#[repr(C)]
pub struct FFieldClass {
    // TODO: remaining members
    /// Field type name, e.g. `StrProperty`, `SetProperty`.
    pub(crate) name: FName,
}

#[derive(Debug)]
#[repr(C)]
pub struct FFieldVariant {
    container: *const c_void,
    b_is_uobject: bool,
}

#[derive(Debug)]
#[repr(C)]
pub struct FField {
    vtable: *const c_void,
    pub class_private: *const FFieldClass,
    pub owner: FFieldVariant,
    pub next: *const FField,
    pub name_private: FName,
    pub flags_private: EObjectFlags,
}

const _: () = assert!(std::mem::offset_of!(FField, next) == 0x20);

const _: () = assert!(std::mem::offset_of!(FField, name_private) == 0x28);

const _: () = assert!(std::mem::size_of::<FField>() == 0x38);
