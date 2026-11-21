//! Authentication token handling and cleanup after native session teardown.

use crate::Chivalry2::Plugins::OnlineSubsystemPlayFab::OnlineAuthInterfacePlayFab::*;
use crate::Chivalry2::Plugins::OnlineSubsystemRedpointEOS::Shared::Authentication::Nodes::LogUserIntoOnlineSubsystemNode::FLogUserIntoOnlineSubsystemNode__GetAuthTokenPatchSite;
use std::ffi::c_void;
use crate::PlatformType::STEAM;
use unchained_macros::{register_patch, Patch};

#[unchained_macros::bind_func_detour(FOnlineAuthPlayFab::EndAuthentication)]
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
        this.EndAuthentication(user_id);
    }
}

// Instruction site in FLogUserIntoOnlineSubsystemNode's native login helper.
// jmp +0x26: skip GetAuthToken and its length check, landing on xor r14b,r14b.
// EBX stays zero from the prologue, so the uninitialized FString is never freed.
#[register_patch(FLogUserIntoOnlineSubsystemNode__GetAuthTokenPatchSite, STEAM)]
static SKIP_STEAM_AUTH_TOKEN: Patch = Patch::bytes(0, &[0xEB, 0x26]);
