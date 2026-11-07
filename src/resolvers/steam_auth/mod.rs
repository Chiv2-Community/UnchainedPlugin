//! Cancel only the throwaway Steam ticket in Redpoint's logged-in predicate.
mod policy;

pub(crate) use policy::signature_required;

use super::{PlatformType, BASE_ADDR};
use crate::ue::FString;
use policy::{take_probe, temporary_token, ProbeScope};
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};

static SUPPORTED_EXECUTABLE: AtomicBool = AtomicBool::new(false);

pub(crate) fn configure(platform: PlatformType, crc: u32) {
    SUPPORTED_EXECUTABLE.store(
        signature_required(
            "RedpointSteamLoginProbe",
            platform == PlatformType::STEAM,
            crc,
        ),
        Ordering::Relaxed,
    );
}

fn supported_build() -> bool {
    // Configured from BuildInfo: PLATFORM/globals do not exist during preinit.
    if !SUPPORTED_EXECUTABLE.load(Ordering::Relaxed) {
        return false;
    }
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    // Cache before either detour changes its prologue. These RVAs intentionally
    // restrict the ABI/layout assumptions to the diagnosed shipping build.
    *SUPPORTED.get_or_init(|| {
        let supported = [(0xeb1a10, 0x15a50a4a), (0xa44fe0, 0x193a2e9b)]
            .into_iter()
            .all(|(offset, fingerprint)| {
                crate::ue::try_read((*BASE_ADDR + offset) as *const [u8; 256])
                    .is_some_and(|code| crc32c::crc32c(&code) == fingerprint)
            });
        if !supported {
            swarn!("Steam ticket cleanup unavailable: unsupported authentication code");
        }
        supported
    })
}

// Covers GetLoginStatus, GetAuthToken and the length-only predicate/free.
define_pattern_resolver!(RedpointSteamLoginProbe, { STEAM: [
    "40 55 53 56 57 41 55 41 56 41 57 48 8D 6C 24 D9 48 81 EC E0 00 00 00 4C 8B E9 48 8B FA 33 DB 49 8B F1 89 5D 6F 4D 8B F8 49 8B 09 48 8B 01 4C 8B 90 18 01 00 00 48 8B 02 8B 90 70 01 00 00 41 FF D2 83 F8 02 75 2D 48 8B 0E 48 8D 55 C7 48 8B 01 4C 8B 88 30 01 00 00 48 8B 07 44 8B 80 70 01 00 00 41 FF D1 BB 01 00 00 00 39 58 08 7E 05 45 32 F6 EB 03 41 B6 01 F6 C3 01 74 0E 48 8B 4D C7 48 85 C9 74 05 E8 ?? ?? ?? ??"
] });
CREATE_HOOK!(RedpointSteamLoginProbe, { supported_build }, NONE, usize, (
    node: *mut c_void, context: *mut *mut c_void,
    completion: *mut c_void, identity: *mut *mut c_void
), {
    if !o_SteamIdentityGetAuthToken.is_enabled() {
        return CALL_ORIGINAL!(RedpointSteamLoginProbe(node, context, completion, identity));
    }
    let identity_ptr = crate::ue::try_read(identity).unwrap_or(std::ptr::null_mut());
    let context_ptr = crate::ue::try_read(context).unwrap_or(std::ptr::null_mut());
    let local_user = if context_ptr.is_null() { None } else {
        crate::ue::try_read(context_ptr.byte_add(0x170).cast::<i32>())
    };
    let _scope = local_user.filter(|_| !identity_ptr.is_null())
        .map(|user| ProbeScope::enter(identity_ptr as usize, user));
    CALL_ORIGINAL!(RedpointSteamLoginProbe(node, context, completion, identity))
});

// Covers the SteamUser field, BLoggedOn and session-ticket slots, and buffer size.
define_pattern_resolver!(SteamIdentityGetAuthToken, { STEAM: [
    "40 53 56 57 48 81 EC 80 04 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 84 24 50 04 00 00 33 FF 48 89 54 24 48 48 89 3A 48 8B F2 48 89 7A 08 48 8B D9 41 83 F8 04 0F 8D ?? ?? ?? ?? 48 8B 89 88 01 00 00 48 85 C9 0F 84 ?? ?? ?? ?? 48 8B 01 FF 50 08 84 C0 0F 84 ?? ?? ?? ?? 48 8B 8B 88 01 00 00 4C 8D 4C 24 40 89 7C 24 40 48 8D 54 24 50 41 B8 00 04 00 00 48 89 7C 24 20 48 8B 01 FF 50 68"
] });
CREATE_HOOK!(SteamIdentityGetAuthToken, { supported_build }, NONE, *mut FString, (
    identity: *mut c_void, result: *mut FString, local_user: i32
), {
    if !take_probe(identity as usize, local_user) {
        return CALL_ORIGINAL!(SteamIdentityGetAuthToken(identity, result, local_user));
    }
    // The game frees this FString. Never hand it a Rust-owned buffer.
    let Some(allocator) = crate::ue::try_gmalloc() else {
        return CALL_ORIGINAL!(SteamIdentityGetAuthToken(identity, result, local_user));
    };
    std::ptr::write(result, FString::new());
    if !(0..4).contains(&local_user) { return result; }
    let steam_user = *identity.byte_add(0x188).cast::<*mut c_void>();
    if steam_user.is_null() { return result; }
    let vtable = *steam_user.cast::<*const usize>();
    let logged_on: unsafe extern "C" fn(*mut c_void) -> bool = std::mem::transmute(*vtable.add(1));
    if !logged_on(steam_user) { return result; }
    let get_ticket: unsafe extern "C" fn(*mut c_void, *mut u8, i32, *mut u32, *const c_void) -> u32 =
        std::mem::transmute(*vtable.add(13));
    let cancel_ticket: unsafe extern "C" fn(*mut c_void, u32) = std::mem::transmute(*vtable.add(17));
    let token = temporary_token(
        |buffer, length| get_ticket(steam_user, buffer.as_mut_ptr(), buffer.len() as i32, length, std::ptr::null()),
        |handle| cancel_ticket(steam_user, handle),
    );
    sdebug!("Redpoint Steam token probe completed with temporary ticket cleanup (nonempty={})", !token.is_empty());
    std::ptr::write(result, FString::from_slice_in(&token, allocator));
    result
});
