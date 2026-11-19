//! Plugin replacement for OnlineSubsystemPlayFab/Source/OnlineSubsystemPlayFab/Public/OnlineAuthInterfacePlayFab.h.

use crate::chivalry2::plugins::online_subsystem_playfab::online_auth_interface_playfab::*;
use std::ffi::c_void;

#[unchained_macros::bind_func_detour(FOnlineAuthPlayFab::end_authentication)]
pub(crate) unsafe fn end_authentication_detour(
    this: *const FOnlineAuthPlayFab,
    user_id: *const c_void,
) {
    // SAFETY: The native caller keeps the auth object and its referenced
    // subsystem valid for this call, using the verified layouts.
    if !unsafe { this.has_session() } {
        sdebug!("Skipping PlayFab logout cleanup after session teardown");
        return;
    }
    unsafe {
        this.end_authentication(user_id);
    }
}
