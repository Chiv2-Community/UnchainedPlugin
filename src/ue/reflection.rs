//! Runtime reflection helpers: resolve a `UFunction`, its native exec-thunk address, or a class's
//! CDO by `/Script/...` path + reflected name via the live UObject system.
//!
//! Split out of `object.rs` so it transfers cleanly onto the modularized `ue` layout on
//! `refactor/unchained-macros` (where `object.rs` is heavily rewritten). Only the resolution source
//! would change there: `get_uobject_from_path` → the branch's `static_find_object` binding.

use super::{UClass, UField, UFunction, UObject, UStruct};

/// Looks up a `UFunction` by name on `class` and its super classes, walking each struct's own
/// `children` (UField) list. No engine calls besides `FName` to string. Returns the first match
/// (name collisions between a function and a non-function UField are not expected on a UClass).
///
/// # Safety
/// `class` must be null or a live `UClass`, and `FNameToString` must be resolved.
pub unsafe fn find_function(class: *const UClass, name: &str) -> Option<*const UFunction> {
    // Bounds guard against a wrong layout turning into an endless walk.
    const MAX_FIELDS: usize = 8192;
    let mut current = class.cast::<UStruct>();
    let mut visited = 0usize;
    while !current.is_null() {
        let mut field: *const UField = (*current).children;
        while !field.is_null() {
            visited += 1;
            if visited > MAX_FIELDS {
                return None;
            }
            if (*field)
                .uobject
                .uobject_base_utility
                .uobject_base
                .name_private
                .to_string()
                == name
            {
                return Some(field.cast::<UFunction>());
            }
            field = (*field).next;
        }
        current = (*current).super_struct;
    }
    None
}

/// Resolves the native exec-thunk address of `name` on the reflected class at `class_path`
/// (e.g. `"/Script/Engine.KismetSystemLibrary"`) by reading `UFunction::func`. This is the general
/// counterpart to patternsleuth's bespoke per-class RegisterNatives table.
///
/// # Safety
/// Must be called after the UObject system is live (postinit) and the `StaticFindObject` /
/// `FNameToString` resolvers are available.
pub unsafe fn find_native(class_path: &str, name: &str) -> Option<usize> {
    let object = crate::game::engine::get_uobject_from_path(class_path, false);
    if object.is_null() {
        return None;
    }
    let function = find_function(object.cast::<UClass>(), name)?;
    Some((*function).func as usize)
}

/// Returns the Class Default Object for the class at `path`, or null. Use this for reading or
/// patching default property values / needing an instance handle (as opposed to [`find_native`],
/// which needs the UClass for reflection).
///
/// # Safety
/// Same preconditions as [`find_native`].
pub unsafe fn cdo_of_path(path: &str) -> *mut UObject {
    let object = crate::game::engine::get_uobject_from_path(path, false);
    if object.is_null() {
        return std::ptr::null_mut();
    }
    (*object.cast::<UClass>()).class_default_object.cast::<UObject>()
}
