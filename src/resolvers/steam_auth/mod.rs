//! Cancel only the throwaway Steam ticket in Redpoint's logged-in predicate.
mod policy;
mod signatures;

pub(crate) use policy::signature_required;

use super::{PlatformType, BASE_ADDR};
use crate::ue::FString;
use policy::{signature_matches, take_probe, temporary_token, ProbeScope};
use signatures::{NODE_SIGNATURE, TOKEN_SIGNATURE};
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};

define_pattern_resolver!(RedpointSteamLoginProbe, { STEAM: [NODE_SIGNATURE] });
define_pattern_resolver!(SteamIdentityGetAuthToken, { STEAM: [TOKEN_SIGNATURE] });

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
        let supported = [(0xeb1a10, NODE_SIGNATURE), (0xa44fe0, TOKEN_SIGNATURE)]
            .into_iter()
            .all(|(offset, signature)| {
                crate::ue::try_read((*BASE_ADDR + offset) as *const [u8; 256])
                    .is_some_and(|code| signature_matches(&code, signature))
            });
        if !supported {
            swarn!("Steam ticket cleanup unavailable: unsupported authentication code");
        }
        supported
    })
}

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
