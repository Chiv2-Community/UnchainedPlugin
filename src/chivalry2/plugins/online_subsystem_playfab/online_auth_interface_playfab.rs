//! Native source: TBL/Plugins/OnlineSubsystemPlayFab/Source/OnlineSubsystemPlayFab/Public/OnlineAuthInterfacePlayFab.h.

use std::ffi::c_void;

define_pattern_resolver!(PlayFabEndAuthentication, [
    "48 89 5C 24 08 55 56 57 41 54 41 55 41 56 41 57 48 8D 6C 24 D9 48 81 EC F0 00 00 00 45 33 E4 4C 8B E9 44 89 65 6F 48 8B CA 48 8B 02 4C 8B FA 41 8B F4 FF 50 28 84 C0 0F 84 ?? ?? ?? ?? 49 8B 8D E0 00 00 00"
]);

// Current Steam/Epic layouts, verified against the native session getter:
// FOnlineAuthPlayFab::PlayFabSubsystem (+0xe0), SessionInterface.Object (+0x140).
#[repr(C)]
pub struct FOnlineAuthPlayFab {
    padding: [u8; 0xe0],
    subsystem: *const SubsystemContext,
}

#[unchained_macros::runtime_bindings]
impl FOnlineAuthPlayFab {
    pub(crate) unsafe fn has_session(self: *const Self) -> bool {
        unsafe { !(*(*self).subsystem).session.is_null() }
    }

    // Shutdown clears SessionInterface before the auth destructor revokes tickets.
    // Skip EndAuthentication, not the remaining destructor cleanup.
    #[unchained_macros::bind_resolved_func(PlayFabEndAuthentication)]
    pub(crate) unsafe fn end_authentication(self: *const Self, user_id: *const c_void) {}
}

#[repr(C)]
struct SubsystemContext {
    padding: [u8; 0x140],
    session: *const c_void,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_layout_matches_supported_builds() {
        assert_eq!(std::mem::offset_of!(FOnlineAuthPlayFab, subsystem), 0xe0);
        assert_eq!(std::mem::offset_of!(SubsystemContext, session), 0x140);
    }

    #[test]
    fn detour_and_original_preserve_the_native_pointer_signature() {
        type EndAuthentication = unsafe fn(*const FOnlineAuthPlayFab, *const c_void);
        let _: EndAuthentication = crate::unchained::patches::auth_token_cleanup::end_authentication_detour;
        let _: EndAuthentication = FOnlineAuthPlayFab::end_authentication;
    }

    #[test]
    fn cleared_session_skips_the_native_cleanup_call() {
        let subsystem = SubsystemContext {
            padding: [0; 0x140],
            session: std::ptr::null(),
        };
        let auth = FOnlineAuthPlayFab {
            padding: [0; 0xe0],
            subsystem: &subsystem,
        };
        // The guard must return without calling the uninitialized trampoline.
        unsafe {
            crate::unchained::patches::auth_token_cleanup::end_authentication_detour(&raw const auth, std::ptr::null());
        }
    }
}
