/// State access for legacy and method-targeted detours.
pub struct DetourBindingRegistration {
    pub name: &'static str,
    pub initialized: fn() -> bool,
    pub enabled: fn() -> bool,
}

inventory::collect!(DetourBindingRegistration);

pub fn detour_is_initialized(name: &str) -> bool {
    inventory::iter::<DetourBindingRegistration>
        .into_iter()
        .any(|binding| binding.name == name && (binding.initialized)())
}

pub fn detour_is_enabled(name: &str) -> bool {
    inventory::iter::<DetourBindingRegistration>
        .into_iter()
        .any(|binding| binding.name == name && (binding.enabled)())
}
use std::collections::BTreeSet;
use std::sync::Mutex;

static CLAIMED_ADDRESSES: Mutex<BTreeSet<usize>> = Mutex::new(BTreeSet::new());

/// Reservation shared by legacy and method-targeted detours.
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
