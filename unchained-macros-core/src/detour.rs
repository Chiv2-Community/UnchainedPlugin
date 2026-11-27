use crate::DetourInfo;

pub fn detour_is_initialized(name: &str) -> bool {
    inventory::iter::<&'static dyn DetourInfo>
        .into_iter()
        .any(|binding| binding.name() == name && binding.initialized())
}

pub fn detour_is_enabled(name: &str) -> bool {
    inventory::iter::<&'static dyn DetourInfo>
        .into_iter()
        .any(|binding| binding.name() == name && binding.enabled())
}

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::sync::OnceLock;

/// Shared physical attachment. Logical callbacks belong to the typed target.

pub struct NativeHook {
    hook: OnceLock<retour::RawDetour>,
    published: AtomicBool,
}

impl NativeHook {
    pub const fn new() -> Self {
        Self {
            hook: OnceLock::new(),
            published: AtomicBool::new(false),
        }
    }

    pub fn detour(&self) -> Option<&retour::RawDetour> {
        self.published
            .load(Ordering::Acquire)
            .then(|| self.hook.get())
            .flatten()
    }

    /// # Safety
    /// Dispatcher ABI must match binding; all native callers must be quiescent.

    pub unsafe fn attach<R: crate::AddressResolver>(
        &'static self,
        binding: &'static crate::FunctionBinding<R>,
        dispatcher: *const (),
    ) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            self.attach_with(binding, dispatcher, |hook| {
                hook.enable()?;

                Ok(())
            })
        }
    }

    unsafe fn attach_with<R: crate::AddressResolver>(
        &'static self,
        binding: &'static crate::FunctionBinding<R>,
        dispatcher: *const (),
        activate: impl FnOnce(&retour::RawDetour) -> Result<(), Box<dyn std::error::Error>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.hook.get().is_none() {
            let address = binding
                .entry_address()
                .ok_or_else(|| format!("Native binding {} is not initialized", binding.name()))?
                .get();

            let claim = claim_detour_address(address)?;

            let hook = unsafe { retour::RawDetour::new(address as *const (), dispatcher)? };

            self.hook.set(hook).map_err(|_| retour::Error::AlreadyInitialized)?;

            claim.commit();

            // Publication can fail after some aliases have received the trampoline.
            // Retain storage and the address claim, but never enable a partially published hook.
            unsafe {
                crate::binding::publish_function_detours(binding, self.hook.get().unwrap())?;
            }

            self.published.store(true, Ordering::Release);
        }

        let hook = self.detour().ok_or("Native trampoline publication previously failed")?;

        if !hook.is_enabled() {
            activate(hook)?;
        }

        Ok(())
    }
}

impl Default for NativeHook {
    fn default() -> Self {
        Self::new()
    }
}

static CLAIMED_ADDRESSES: Mutex<BTreeSet<usize>> = Mutex::new(BTreeSet::new());

/// Reservation retained by an attached detour.
#[doc(hidden)]

pub struct DetourAddressClaim {
    address: usize,
    committed: bool,
}

impl DetourAddressClaim {
    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for DetourAddressClaim {
    fn drop(&mut self) {
        if !self.committed {
            CLAIMED_ADDRESSES
                .lock()
                .expect("Detour address registry poisoned")
                .remove(&self.address);
        }
    }
}

/// Reserve by address, so differently named aliases cannot patch one target twice.
#[doc(hidden)]

pub fn claim_detour_address(address: usize) -> Result<DetourAddressClaim, String> {
    if address == 0 {
        return Err("Native detour address is null".into());
    }

    if !CLAIMED_ADDRESSES
        .lock()
        .map_err(|_| "Detour address registry poisoned")?
        .insert(address)
    {
        return Err(format!("A detour already owns native address {address:#x}"));
    }

    Ok(DetourAddressClaim {
        address,
        committed: false,
    })
}

#[cfg(test)]

mod tests {

    use super::*;
    use crate::{AddressResolver, FunctionBinding, Resolver};
    use std::num::NonZeroUsize;

    struct Backend;

    impl AddressResolver for Backend {
        type Context = ();

        unsafe fn resolve(&self, _: &()) -> Result<NonZeroUsize, String> {
            unreachable!()
        }
    }

    static RESOLVER: Resolver<Backend> = Resolver {
        name: "activation retry",
        implementation: Backend,
        filter: None,
    };

    static BINDING: FunctionBinding<Backend> = FunctionBinding::new("activation retry", &RESOLVER);

    static HOOK: NativeHook = NativeHook::new();

    inventory::submit! {
        &BINDING as &dyn crate::binding::FunctionInfo
    }

    #[inline(never)]

    unsafe extern "C" fn original(value: usize) -> usize {
        std::hint::black_box(value).wrapping_add(1)
    }

    unsafe extern "C" fn replacement(value: usize) -> usize {
        value + 10
    }

    #[inline(never)]

    unsafe extern "C" fn unpublished_entry(value: usize) -> usize {
        std::hint::black_box(value).wrapping_add(23)
    }

    struct RejectingAlias;

    impl crate::binding::FunctionInfo for RejectingAlias {
        fn entry_address(&self) -> Option<NonZeroUsize> {
            NonZeroUsize::new(unpublished_entry as *const () as usize)
        }

        unsafe fn publish_detour(&self, _: &'static retour::RawDetour) -> Result<(), String> {
            Err("injected alias publication failure".into())
        }
    }

    inventory::submit! { &RejectingAlias as &dyn crate::binding::FunctionInfo }

    #[test]

    fn partial_publication_never_enables_the_hook_on_retry() {
        static OWNER: FunctionBinding<Backend> = FunctionBinding::new("publication failure", &RESOLVER);

        static FAILED: NativeHook = NativeHook::new();

        unsafe {
            OWNER
                .initialize_address(NonZeroUsize::new(unpublished_entry as *const () as usize).unwrap())
                .unwrap();

            let attach = || {
                FAILED.attach_with(&OWNER, replacement as *const (), |_| {
                    panic!("activated unpublished hook")
                })
            };

            assert!(attach()
                .unwrap_err()
                .to_string()
                .contains("injected alias publication failure"));

            assert_ne!(OWNER.address(), OWNER.entry_address());

            assert!(attach()
                .unwrap_err()
                .to_string()
                .contains("publication previously failed"));

            assert!(FAILED.detour().is_none());

            assert!(!FAILED.hook.get().unwrap().is_enabled());
        }
    }

    #[test]

    fn stored_detour_retries_failed_physical_activation_before_reporting_success() {
        unsafe {
            BINDING
                .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
                .unwrap();

            let error = HOOK
                .attach_with(&BINDING, replacement as *const (), |_| {
                    Err("injected activation failure".into())
                })
                .unwrap_err();

            assert!(error.to_string().contains("injected activation failure"));

            let physical = HOOK.detour().unwrap();

            assert!(!physical.is_enabled());

            assert_ne!(BINDING.address(), BINDING.entry_address());

            assert_eq!(original(1), 2);

            HOOK.attach(&BINDING, replacement as *const ()).unwrap();

            assert!(physical.is_enabled());

            assert!(std::ptr::eq(physical, HOOK.detour().unwrap()));

            assert_eq!(original(1), 11);

            let bypass: unsafe extern "C" fn(usize) -> usize = std::mem::transmute(BINDING.address().unwrap().get());

            assert_eq!(bypass(1), 2);
        }
    }
}
