//! Runtime registration shared by generated hooks, patches, and offset providers.

use std::collections::HashMap;

pub type CreateHookFn = unsafe fn(usize, bool)
    -> Result<Option<usize>, Box<dyn std::error::Error>>;

pub struct HookRegistration {
    /// Rust detour function name.
    pub name: &'static str,
    pub binding: &'static crate::FunctionBinding,
    pub hook_fn: CreateHookFn,
    pub condition: fn() -> bool,
    pub detour: fn() -> Option<&'static retour::RawDetour>,
}

pub struct PatchRegistration {
    /// Annotated factory/static item name; unique across registered patches.
    pub name: &'static str,
    /// Native resolver descriptor, including its selection filter.
    pub resolver: &'static crate::ResolverDescriptor,
    pub factory: fn(u64) -> crate::Patch,
    pub selector: Option<fn() -> std::any::TypeId>,
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
