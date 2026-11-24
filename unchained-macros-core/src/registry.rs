//! Typed registrations indexed through backend-independent discovery records.
use std::any::{Any, TypeId};
use crate::{AddressResolver, FunctionBinding, Resolver, SelectorSnapshot};
use crate::resolver::ResolverInfo;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::detour::NativeHook;
use crate::binding::FunctionInfo;

pub type CreateHookFn = unsafe fn(bool) -> Result<(), Box<dyn std::error::Error>>;

/// Process-lifetime replacement ownership. Attach only during quiescent startup.
pub struct HookRegistration {
    pub name: &'static str,
    binding: &'static dyn FunctionInfo,
    hook: &'static NativeHook,
    install: CreateHookFn,
    initialized: AtomicBool,
    pub condition: fn() -> bool,
}
impl HookRegistration {
    pub const fn new<R: AddressResolver>(
        name: &'static str, binding: &'static FunctionBinding<R>, hook: &'static NativeHook,
        install: CreateHookFn, condition: fn() -> bool,
    ) -> Self {
        Self { name, binding, hook, install, condition, initialized: AtomicBool::new(false) }
    }
    pub fn binding<R: AddressResolver>(&self) -> Option<&'static FunctionBinding<R>> {
        (self.binding as &dyn Any).downcast_ref()
    }
    pub fn resolver(&self) -> &'static dyn ResolverInfo { self.binding.resolver() }
    /// # Safety
    /// Attach during quiescent startup. The installer must publish this hook's
    /// ABI-correct process-lifetime replacement, rejecting duplicates before
    /// publication. It must not publish the callback on failure.
    /// Do not attach concurrently.
    pub unsafe fn attach(&self, activate: bool) -> Result<(), Box<dyn std::error::Error>> {
        if self.initialized() { return Err("Detour callback is already initialized".into()); }
        unsafe { (self.install)(activate)?; }
        self.initialized.store(true, Ordering::Release);
        Ok(())
    }
    pub fn initialized(&self) -> bool { self.initialized.load(Ordering::Acquire) }
    pub fn detour(&self) -> Option<&'static retour::RawDetour> {
        self.initialized().then(|| self.hook.detour()).flatten()
    }
    pub fn enabled(&self) -> bool {
        self.detour().is_some_and(|hook| hook.is_enabled())
            && self.hook.replacement_address().is_some()
    }
    pub fn set_enabled(&self, enabled: bool) {
        if self.initialized() { self.hook.set_enabled(enabled); }
    }
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
pub trait PatchInfo: Any + Send + Sync {
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

inventory::collect!(&'static HookRegistration);
inventory::collect!(&'static dyn PatchInfo);

pub fn hook_inventory_for<R: AddressResolver>() -> impl Iterator<Item = &'static HookRegistration> {
    inventory::iter::<&'static HookRegistration>.into_iter().copied()
        .filter(|entry| entry.binding::<R>().is_some())
}
pub fn patch_inventory_for<R: AddressResolver>() -> impl Iterator<Item = &'static PatchRegistration<R>> {
    inventory::iter::<&'static dyn PatchInfo>.into_iter().filter_map(|entry| (*entry as &dyn Any).downcast_ref())
}
