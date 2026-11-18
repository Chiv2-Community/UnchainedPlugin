#![allow(dead_code)]
mod array;
mod archive;
pub mod asset_registry;
pub mod core;
pub mod core_uobject;
pub mod engine;
pub mod tbl;
pub mod kismet;
mod malloc;
mod map;
mod memory;
mod object;
mod object_ptr;
pub mod object_array;
pub mod stubs;
pub mod sync;

pub use array::*;
pub use archive::*;
pub use asset_registry::*;
pub use core::*;
pub use core_uobject::*;
pub use engine::*;
pub use tbl::*;
pub use malloc::*;
pub use map::*;
pub use memory::*;
pub use object::*;
pub use object_ptr::*;
pub use stubs::*;

use std::ffi::c_void;

use crate::globals;

pub type FnFFrameStep =
    unsafe extern "system" fn(stack: &mut kismet::FFrame, *mut UObject, result: *mut c_void);
pub type FnFFrameStepExplicitProperty = unsafe extern "system" fn(
    stack: &mut kismet::FFrame,
    result: *mut c_void,
    property: *const FProperty,
);
pub type FnFNameToString = unsafe extern "system" fn(&FName, &mut FString);
pub type FnFNameCtorWchar = unsafe extern "system" fn(&mut FName, *const TCHAR, EFindName);

pub type FnUObjectBaseUtilityGetPathName =
    unsafe extern "system" fn(&UObjectBase, Option<&UObject>, &mut FString);

#[derive(Debug, Default)]
#[repr(C)]
pub struct FVector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct FLinearColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
