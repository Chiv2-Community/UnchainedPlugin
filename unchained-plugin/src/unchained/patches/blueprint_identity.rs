//! Optional result overrides; these are detours, not observation callbacks.
use crate::Chivalry2::Engine::{UKismetSystemLibrary, UObject};

#[unchained_macros_core::bind_func_entry_listener(UKismetSystemLibrary::IsDedicatedServer)]
unsafe fn observe_dedicated_server(_context: &*mut UObject) {}

#[unchained_macros_core::bind_func_entry_listener(UKismetSystemLibrary::IsServer)]
unsafe fn observe_server(_context: &*mut UObject) {}

#[unchained_macros_core::bind_func_entry_listener(UKismetSystemLibrary::IsStandalone)]
unsafe fn observe_standalone(_context: &*mut UObject) {}

#[unchained_macros_core::bind_func_detour(UKismetSystemLibrary::IsDedicatedServer)]
unsafe fn force_dedicated_server(context: *mut UObject) -> bool {
    unsafe { UKismetSystemLibrary::IsDedicatedServer(context); }
    true
}

#[cfg(feature = "force_kismet_is_server")]
#[unchained_macros_core::bind_func_detour(UKismetSystemLibrary::IsServer)]
unsafe fn force_server(context: *mut UObject) -> bool {
    unsafe { UKismetSystemLibrary::IsServer(context); }
    true
}

#[cfg(feature = "force_kismet_is_standalone")]
#[unchained_macros_core::bind_func_detour(UKismetSystemLibrary::IsStandalone)]
unsafe fn force_standalone(context: *mut UObject) -> bool {
    unsafe { UKismetSystemLibrary::IsStandalone(context); }
    true
}
