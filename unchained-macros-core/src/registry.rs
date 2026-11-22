//! Typed registrations indexed through backend-independent discovery records.
use std::any::{Any, TypeId};
use crate::{AddressResolver, FunctionBinding, Resolver, SelectorSnapshot};
use crate::resolver::ResolverInfo;

pub type CreateHookFn = unsafe fn(bool) -> Result<(), Box<dyn std::error::Error>>;

pub struct HookRegistration<R: AddressResolver> {
    pub name: &'static str,
    pub binding: &'static FunctionBinding<R>,
    pub hook_fn: CreateHookFn,
    pub condition: fn() -> bool,
    pub detour: fn() -> Option<&'static retour::RawDetour>,
}

pub struct PatchRegistration<R: AddressResolver> {
    pub name: &'static str,
    pub resolver: &'static Resolver<R>,
    pub factory: fn(u64) -> crate::Patch,
    pub selector: Option<fn() -> TypeId>,
    pub applies_to: fn(&dyn Any) -> bool,
    pub validate_group: fn(&[&dyn PatchInfo]) -> Result<(), String>,
}

#[doc(hidden)]
pub trait PatchInfo: Send + Sync {
    fn name(&self) -> &'static str;
    fn resolver(&self) -> &dyn ResolverInfo;
    fn selector(&self) -> Option<fn() -> TypeId>;
    fn applies_to(&self, selection: &dyn Any) -> bool;
    fn validate_group(&self, group: &[&dyn PatchInfo]) -> Result<(), String>;
}
impl<R: AddressResolver> PatchInfo for PatchRegistration<R> {
    fn name(&self) -> &'static str { self.name }
    fn resolver(&self) -> &dyn ResolverInfo { self.resolver }
    fn selector(&self) -> Option<fn() -> TypeId> { self.selector }
    fn applies_to(&self, selection: &dyn Any) -> bool { (self.applies_to)(selection) }
    fn validate_group(&self, group: &[&dyn PatchInfo]) -> Result<(), String> { (self.validate_group)(group) }
}
impl<R: AddressResolver> PatchRegistration<R> {
    pub fn supports(&self, snapshot: &SelectorSnapshot) -> bool { snapshot.supports_patch(self) }
}

#[doc(hidden)]
pub trait HookInfo: Send + Sync {
    fn name(&self) -> &'static str;
    fn detour(&self) -> Option<&'static retour::RawDetour>;
}
impl<R: AddressResolver> HookInfo for HookRegistration<R> {
    fn name(&self) -> &'static str { self.name }
    fn detour(&self) -> Option<&'static retour::RawDetour> { (self.detour)() }
}

#[doc(hidden)]
pub struct ErasedHookRegistration {
    pub info: &'static dyn HookInfo,
    value: &'static (dyn Any + Send + Sync),
}
impl ErasedHookRegistration {
    pub const fn new<R: AddressResolver>(hook: &'static HookRegistration<R>) -> Self { Self { info: hook, value: hook } }
}
#[doc(hidden)]
pub struct ErasedPatchRegistration {
    pub info: &'static dyn PatchInfo,
    value: &'static (dyn Any + Send + Sync),
}
impl ErasedPatchRegistration {
    pub const fn new<R: AddressResolver>(patch: &'static PatchRegistration<R>) -> Self { Self { info: patch, value: patch } }
}
inventory::collect!(ErasedHookRegistration);
inventory::collect!(ErasedPatchRegistration);

pub fn hook_inventory_for<R: AddressResolver>() -> impl Iterator<Item = &'static HookRegistration<R>> {
    inventory::iter::<ErasedHookRegistration>.into_iter().filter_map(|entry| entry.value.downcast_ref())
}
pub fn patch_inventory_for<R: AddressResolver>() -> impl Iterator<Item = &'static PatchRegistration<R>> {
    inventory::iter::<ErasedPatchRegistration>.into_iter().filter_map(|entry| entry.value.downcast_ref())
}
