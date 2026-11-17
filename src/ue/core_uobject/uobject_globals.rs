//! Global UE functions from UObjectGlobals.h, not UObject methods.
use crate::ue::{FUObjectSerializeContext, TCHAR, UClass, UObject, UPackageMap};
use unchained_macros::bind_resolver_func;

/// # Safety
/// Call on the engine thread. Class/package must be live or valid null defaults;
/// name must be a NUL-terminated UTF-16 path. The result is borrowed.
#[bind_resolver_func(crate::resolvers::asset_registry::StaticFindObject)]
pub unsafe fn static_find_object(class: *mut UClass, package: *mut UObject, name: *const TCHAR, exact_class: bool) -> *mut UObject {}

/// # Safety
/// Call on the engine thread. Pointers must be valid native arguments or the
/// documented null defaults; string pointers must be NUL-terminated UTF-16.
/// The result is borrowed and may require subsequent engine GC protection.
#[bind_resolver_func(crate::resolvers::asset_registry::StaticLoadObject)]
pub unsafe fn static_load_object(class: *mut UClass, outer: *mut UObject, name: *const TCHAR, filename: *const TCHAR, load_flags: u32, sandbox: *mut UPackageMap, allow_object_reconciliation: bool, serialize_context: *mut FUObjectSerializeContext) -> *mut UObject {}
