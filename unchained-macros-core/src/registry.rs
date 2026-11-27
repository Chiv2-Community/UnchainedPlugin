//! Typed registrations indexed through backend-independent discovery records.
use crate::resolver::ResolverInfo;
use crate::{AddressResolver, Resolver, SelectorSnapshot};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    any::{Any, TypeId},
    borrow::Borrow,
};

/// A replacement and the target whose callback contract it satisfies.
/// Inventory is optional; attachment always uses the referenced target.
///
/// Replacements cannot narrow borrowed arguments or change the ABI:
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe extern "C" fn retains(_: &'static i32) {}
/// fn invalid<T: SupportsDetour<Callback = unsafe extern "C" fn(&i32)>>(target: &'static T) {
///     let _ = DetourRegistration::new("retains", target, retains, || true);
/// }
/// ```
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe fn rust_abi(_: u32) {}
/// fn invalid<T: SupportsDetour<Callback = unsafe extern "C" fn(u32)>>(target: &'static T) {
///     let _ = DetourRegistration::new("rust ABI", target, rust_abi, || true);
/// }
/// ```
pub struct DetourRegistration<T: crate::SupportsDetour + 'static, State = AtomicBool> {
    pub name: &'static str,
    target: &'static T,
    callback: T::Callback,
    condition: fn() -> bool,
    initialized: State,
}
impl<T: crate::SupportsDetour> DetourRegistration<T> {
    pub const fn new(name: &'static str, target: &'static T, callback: T::Callback, condition: fn() -> bool) -> Self {
        Self {
            name,
            target,
            callback,
            condition,
            initialized: AtomicBool::new(false),
        }
    }
    pub const fn target(&self) -> &'static T {
        self.target
    }

    /// Name only the mutable storage in generated inventory statics; the compiler
    /// can then infer and promote the immutable target/callback pair (including HRTBs).
    /// # Safety
    /// `initialized` must be fresh, false, and reserved solely for this registration.
    #[doc(hidden)]
    pub const unsafe fn with_inventory_state(
        self,
        initialized: &'static AtomicBool,
    ) -> DetourRegistration<T, &'static AtomicBool> {
        DetourRegistration {
            name: self.name,
            target: self.target,
            callback: self.callback,
            condition: self.condition,
            initialized,
        }
    }
}
/// Object-safe discovery/control surface. Callback types stay on the registration.
pub trait DetourInfo: Sync {
    fn name(&self) -> &'static str;
    fn resolver(&self) -> &'static dyn ResolverInfo;
    fn condition(&self) -> bool;
    /// # Safety
    /// Attach during quiescent startup, satisfying target readiness and the callback's
    /// ABI, aliasing and lifetime contracts. Do not attach concurrently.
    unsafe fn attach(&self, activate: bool) -> Result<(), Box<dyn std::error::Error>>;
    fn initialized(&self) -> bool;
    fn enabled(&self) -> bool;
    fn set_enabled(&self, enabled: bool);
    fn detour(&self) -> Option<&'static retour::RawDetour>;
}
impl<T: crate::SupportsDetour, S: Borrow<AtomicBool> + Sync> DetourInfo for DetourRegistration<T, S> {
    fn name(&self) -> &'static str {
        self.name
    }
    fn resolver(&self) -> &'static dyn ResolverInfo {
        self.target.resolver()
    }
    fn condition(&self) -> bool {
        self.target.activation(true, self.condition)
    }
    unsafe fn attach(&self, activate: bool) -> Result<(), Box<dyn std::error::Error>> {
        if self.initialized() {
            return Err("Detour callback is already initialized".into());
        }
        unsafe {
            self.target.register_detour(self.callback, activate, self.condition)?;
        }
        self.initialized.borrow().store(true, Ordering::Release);
        Ok(())
    }
    fn initialized(&self) -> bool {
        self.initialized.borrow().load(Ordering::Acquire)
    }
    fn detour(&self) -> Option<&'static retour::RawDetour> {
        self.initialized().then(|| self.target.physical_detour()).flatten()
    }
    fn enabled(&self) -> bool {
        self.detour().is_some_and(|hook| hook.is_enabled()) && self.target.replacement_slot().get().is_some()
    }
    fn set_enabled(&self, enabled: bool) {
        if self.initialized() {
            self.target.replacement_slot().set_enabled(enabled);
        }
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
    fn name(&self) -> &'static str {
        self.name
    }
    fn resolver(&self) -> &dyn ResolverInfo {
        self.resolver
    }
    fn selector(&self) -> Option<fn() -> TypeId> {
        self.selector
    }
    fn applies_to(&self, selection: &dyn Any) -> bool {
        (self.applies_to)(selection)
    }
    fn validate_group(&self, group: &[&dyn PatchInfo]) -> Result<(), String> {
        (self.validate_group)(group)
    }
}
impl<R: AddressResolver> PatchRegistration<R> {
    pub fn supports(&self, snapshot: &SelectorSnapshot) -> bool {
        snapshot.supports_patch(self)
    }
}

inventory::collect!(&'static dyn DetourInfo);
inventory::collect!(&'static dyn PatchInfo);

pub fn detour_inventory_for<R: AddressResolver>() -> impl Iterator<Item = &'static dyn DetourInfo> {
    inventory::iter::<&'static dyn DetourInfo>
        .into_iter()
        .copied()
        .filter(|entry| entry.resolver().backend() == TypeId::of::<R>())
}
pub fn patch_inventory_for<R: AddressResolver>() -> impl Iterator<Item = &'static PatchRegistration<R>> {
    inventory::iter::<&'static dyn PatchInfo>
        .into_iter()
        .filter_map(|entry| (*entry as &dyn Any).downcast_ref())
}
