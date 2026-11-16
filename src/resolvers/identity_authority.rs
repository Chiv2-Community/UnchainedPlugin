//! Server-owned typed IDs. Client RPCs never supply identity after opt-in.
use super::game_identity::{create_native_id, NativeIdentityRef};
use crate::tools::hook_globals::cli_args;
use server_browser_client::models::{game_platform_identity::Provider, GameAdmissionClaims};
use std::ffi::c_void;

define_pattern_resolver!(IdentitySetBase, [
    "48 89 5C 24 08 57 48 83 EC 20 48 8B FA 48 8B D9 8B 49 28 BA 13 00 00 00 E8 ?? ?? ?? ?? 48 8D 8B 88 02 00 00 48 8B D7 48 8B 01"
]);
CREATE_HOOK!(IdentitySetBase, ACTIVE, (), (state: *mut u8, id: *const NativeIdentityRef), {});

define_pattern_resolver!(IdentitySetVerifiedPlayFab, [
    "48 89 54 24 10 55 53 56 57 41 54 41 56 41 57 48 8D AC 24 10 FF FF FF 48 81 EC F0 01 00 00 45 33 FF 48 8B F1 48 8B 0A 4C 8B E2"
]);
CREATE_HOOK!(IdentitySetVerifiedPlayFab, ACTIVE, (), (state: *mut u8, id: *const NativeIdentityRef), {});

define_pattern_resolver!(IdentitySetVerifiedPlatform, [
    "48 89 5C 24 08 57 48 83 EC 30 48 8B D9 48 8B FA 48 8B 0A 48 85 C9 0F 84 ?? ?? ?? ?? 48 8B 01 FF 50 28 84 C0"
]);
CREATE_HOOK!(IdentitySetVerifiedPlatform, ACTIVE, (), (state: *mut u8, id: *const NativeIdentityRef), {});

// Hook the server implementation, not its RPC wrapper or property replication.
// Native client-to-server calls still deserialize and release their parameters.
define_pattern_resolver!(IdentityPlatformUpdate, [
    "48 89 5C 24 08 57 48 83 EC 20 48 8B 99 60 02 00 00 48 8B FA 48 85 DB 74 2F E8 ?? ?? ?? ?? 48 8B 4B 10 4C 8D 40 30"
]);
CREATE_HOOK!(IdentityPlatformUpdate, ACTIVE, NONE, (), (controller: *mut c_void, id: *const c_void), {
    if cli_args().validated_player_identity && cli_args().is_server() {
        return;
    }
    CALL_ORIGINAL!(IdentityPlatformUpdate(controller, id));
});

pub fn available() -> bool {
    o_IdentitySetBase.is_enabled()
        && o_IdentitySetVerifiedPlayFab.is_enabled()
        && o_IdentitySetVerifiedPlatform.is_enabled()
        && o_IdentityPlatformUpdate.is_enabled()
}

pub struct VerifiedIds {
    base: NativeIdentityRef,
    playfab: NativeIdentityRef,
    platform: NativeIdentityRef,
}

impl VerifiedIds {
    /// Construct all wrappers before changing any PlayerState field. Call only
    /// with claims returned by the offline verifier, on the game thread.
    pub unsafe fn prepare(claims: &GameAdmissionClaims) -> Option<Self> {
        if !available() { return None; }
        let ids = &claims.identities;
        let platform = match ids.platform.provider {
            Provider::Steam => "Steam",
            Provider::Eos => "RedpointEOS",
        };
        Some(Self {
            base: unsafe { create_native_id("RedpointEOS", &ids.eos_product_user_id) }?,
            playfab: unsafe { create_native_id("PlayFab", &ids.playfab) }?,
            platform: unsafe { create_native_id(platform, &ids.platform.id) }?,
        })
    }

    /// Native setters own their references and mark replication properties dirty.
    /// The caller must first revalidate the weak connection/PlayerState witness.
    pub unsafe fn assign(&self, state: *mut u8) {
        CALL_ORIGINAL!(IdentitySetBase(state, &self.base));
        CALL_ORIGINAL!(IdentitySetVerifiedPlayFab(state, &self.playfab));
        CALL_ORIGINAL!(IdentitySetVerifiedPlatform(state, &self.platform));
    }
}
