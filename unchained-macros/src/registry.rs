//! Runtime registration shared by generated hooks, patches, and offset providers.

use std::collections::HashMap;

pub type CreateHookFn = unsafe fn(usize, bool)
    -> Result<Option<usize>, Box<dyn std::error::Error>>;

pub struct HookRegistration {
    pub name: &'static str,
    pub hook_fn: CreateHookFn,
    pub condition: fn() -> bool,
    pub initialized: fn() -> bool,
    pub enabled: fn() -> bool,
}

pub type RegistrationPredicate = fn() -> bool;
pub type CreatePatchFn = unsafe fn(usize, &HashMap<String, u64>)
    -> Result<(), Box<dyn std::error::Error>>;

pub struct PatchRegistration {
    pub name: &'static str,
    pub tag: &'static str,
    pub patch_fn: CreatePatchFn,
    pub enabled_fn: RegistrationPredicate,
}

/// A named group of offsets.
pub struct OffsetRegisty {
    pub name: &'static str,
    pub map: fn() -> HashMap<String, u64>,
}

inventory::collect!(HookRegistration);
inventory::collect!(PatchRegistration);
inventory::collect!(OffsetRegisty);
