//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/Field.h>.

use crate::Chivalry2::Engine::*;

use std::ffi::c_void;

#[unchained_macros_core::runtime_bindings]
#[derive(Debug)]
#[repr(C)]
pub struct FFieldClass {
    // TODO: remaining members
    /// Field type name, e.g. `StrProperty`, `SetProperty`.
    pub(crate) name: FName,
}

#[unchained_macros_core::runtime_bindings]
#[derive(Debug)]
#[repr(C)]
pub struct FFieldVariant {
    container: *const c_void,
    b_is_uobject: bool,
}

#[unchained_macros_core::runtime_bindings]
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
