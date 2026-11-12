use std::ffi::c_void;

// Current Steam/Epic layouts, verified against the native session getter:
// FOnlineAuthPlayFab::PlayFabSubsystem (+0xe0), SessionInterface.Object (+0x140).
#[repr(C)]
struct AuthContext<'a> {
    padding: [u8; 0xe0],
    subsystem: &'a SubsystemContext,
}

#[repr(C)]
struct SubsystemContext {
    padding: [u8; 0x140],
    session: *const c_void,
}

define_pattern_resolver!(PlayFabEndAuthentication, [
    "48 89 5C 24 08 55 56 57 41 54 41 55 41 56 41 57 48 8D 6C 24 D9 48 81 EC F0 00 00 00 45 33 E4 4C 8B E9 44 89 65 6F 48 8B CA 48 8B 02 4C 8B FA 41 8B F4 FF 50 28 84 C0 0F 84 ?? ?? ?? ?? 49 8B 8D E0 00 00 00"
]);
// Shutdown clears SessionInterface before the auth destructor revokes tickets.
// EndAuthentication assumes that interface exists; the remaining destructor cleanup still runs.
CREATE_HOOK!(PlayFabEndAuthentication, ACTIVE, NONE, (), (
    auth: *const c_void, user_id: *const c_void
), {
    // retour requires a lifetime-erased pointer at the detour boundary.
    // SAFETY: The native caller keeps the auth object and its referenced subsystem
    // valid for this call, using the verified layouts above.
    let context = unsafe { &*auth.cast::<AuthContext<'_>>() };
    if context.subsystem.session.is_null() {
        sdebug!("Skipping PlayFab logout cleanup after session teardown");
        return;
    }
    CALL_ORIGINAL!(PlayFabEndAuthentication(auth, user_id));
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_layout_matches_supported_builds() {
        assert_eq!(std::mem::offset_of!(AuthContext<'_>, subsystem), 0xe0);
        assert_eq!(std::mem::offset_of!(SubsystemContext, session), 0x140);
    }
}
