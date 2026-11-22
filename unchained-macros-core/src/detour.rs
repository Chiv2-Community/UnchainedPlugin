use crate::registry::HookInfo;

pub fn detour_is_initialized(name: &str) -> bool {
    inventory::iter::<&'static dyn HookInfo>
        .into_iter()
        .any(|binding| binding.name() == name && binding.detour().is_some())
}

pub fn detour_is_enabled(name: &str) -> bool {
    inventory::iter::<&'static dyn HookInfo>
        .into_iter()
        .any(|binding| binding.name() == name && binding.detour().is_some_and(retour::RawDetour::is_enabled))
}
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Construct and publish a process-lifetime detour before evaluating activation.
///
/// # Safety
/// The replacement must match the binding's verified native ABI. Attach while
/// native callers are quiescent; storage and trampolines remain live permanently.
#[doc(hidden)]
pub unsafe fn attach_function_detour<R: crate::AddressResolver>(
    binding: &'static crate::FunctionBinding<R>,
    storage: &'static OnceLock<retour::RawDetour>,
    replacement: *const (),
    auto_activate: bool,
    condition: fn() -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let address = binding.entry_address()
        .ok_or_else(|| format!("Native binding {} is not initialized", binding.name()))?.get();
    if storage.get().is_some() { return Err(retour::Error::AlreadyInitialized.into()); }
    let claim = claim_detour_address(address)?;
    let detour = unsafe { retour::RawDetour::new(address as *const (), replacement)? };
    storage.set(detour).map_err(|_| retour::Error::AlreadyInitialized)?;
    let hook = storage.get().expect("Hook was just initialized");
    claim.commit();
    unsafe { crate::binding::publish_function_detours(address, hook)?; }
    if auto_activate && condition() { unsafe { hook.enable()?; } }
    Ok(())
}

static CLAIMED_ADDRESSES: Mutex<BTreeSet<usize>> = Mutex::new(BTreeSet::new());

/// Reservation retained by an attached detour.
#[doc(hidden)]
pub struct DetourAddressClaim {
    address: usize,
    committed: bool,
}

impl DetourAddressClaim {
    pub fn commit(mut self) { self.committed = true; }
}

impl Drop for DetourAddressClaim {
    fn drop(&mut self) {
        if !self.committed {
            CLAIMED_ADDRESSES.lock().expect("Detour address registry poisoned").remove(&self.address);
        }
    }
}

/// Reserve by address, so differently named aliases cannot patch one target twice.
#[doc(hidden)]
pub fn claim_detour_address(address: usize) -> Result<DetourAddressClaim, String> {
    if address == 0 { return Err("Native detour address is null".into()); }
    if !CLAIMED_ADDRESSES.lock().map_err(|_| "Detour address registry poisoned")?.insert(address) {
        return Err(format!("A detour already owns native address {address:#x}"));
    }
    Ok(DetourAddressClaim { address, committed: false })
}
