//! Runtime registration shared by generated hooks, patches, and offset providers.

use std::collections::HashMap;

pub type CreateHookFn = unsafe fn(usize, bool)
    -> Result<Option<usize>, Box<dyn std::error::Error>>;

pub struct HookRegistration {
    pub name: &'static str,
    pub hook_fn: CreateHookFn,
    pub condition: fn() -> bool,
    pub detour: fn() -> Option<&'static retour::RawDetour>,
}

pub struct PatchRegistration {
    pub name: &'static str,
    pub resolver: &'static str,
    pub factory: fn(u64) -> crate::Patch,
    /// Selection applicability only; runtime conditions must not affect scan roots.
    pub applies_to: fn(&dyn std::any::Any) -> bool,
    pub validate_group: fn(&[&PatchRegistration]) -> Result<(), String>,
}

/// A named group of offsets.
pub struct OffsetRegisty {
    pub name: &'static str,
    pub map: fn() -> HashMap<String, u64>,
}

inventory::collect!(HookRegistration);
inventory::collect!(PatchRegistration);
inventory::collect!(OffsetRegisty);
