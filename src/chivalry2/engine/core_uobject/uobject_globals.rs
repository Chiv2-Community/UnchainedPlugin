//! UE 4.25-plus: [UObject globals](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/UObjectGlobals.h).

//! Global UE functions from UObjectGlobals.h, not UObject methods.
use crate::chivalry2::engine::{FUObjectSerializeContext, TCHAR, UClass, UObject, UPackageMap};
use unchained_macros::bind_resolved_func;
mod helpers;
pub use helpers::*;

define_pattern_resolver!(StaticFindObject,["48 89 5C 24 08 48 89 74 24 18 55 57 41 54 41 56 41 57 48 8B EC 48 83 EC 60 80 3D 98 BB"]);

define_pattern_resolver!(StaticLoadObject,["40 55 53 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 28 FA FF FF 48 81 EC D8 06 00 00 48 8B 05 25 5E 9F 03 48 33 C4 48 89 85 C0 05 00 00 48"]);

// UObject* StaticLoadObject(UClass* ObjectClass, UObject* InOuter, const TCHAR* Name, const TCHAR* Filename, uint32 LoadFlags, UPackageMap* Sandbox, bool bAllowObjectReconciliation, FUObjectSerializeContext* InSerializeContext)
// The resolved target uses FUObjectSerializeContext, not FLinkerInstancingContext.

/// # Safety
/// Call on the engine thread. Class/package must be live or valid null defaults;
/// name must be a NUL-terminated UTF-16 path. The result is borrowed.
#[bind_resolved_func(self::StaticFindObject)]
pub unsafe fn static_find_object(class: *mut UClass, package: *mut UObject, name: *const TCHAR, exact_class: bool) -> *mut UObject {}

/// # Safety
/// Call on the engine thread. Pointers must be valid native arguments or the
/// documented null defaults; string pointers must be NUL-terminated UTF-16.
/// The result is borrowed and may require subsequent engine GC protection.
#[bind_resolved_func(self::StaticLoadObject)]
pub unsafe fn static_load_object(class: *mut UClass, outer: *mut UObject, name: *const TCHAR, filename: *const TCHAR, load_flags: u32, sandbox: *mut UPackageMap, allow_object_reconciliation: bool, serialize_context: *mut FUObjectSerializeContext) -> *mut UObject {}
