use std::ffi::c_void;

define_pattern_resolver!(FilterConsoleCommands, [
    "48 89 5C 24 08 57 48 83 EC 20 65 48 8B 04 25 58 00 00 00 48 8B F9 8B 15 ?? ?? ?? ?? B9 78 00 00 00 4C 8B 04 D0 42 8B 04 01 39 05 ?? ?? ?? ?? 0F 8F BD 00 00 00"
]);

type ArchiveCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);

CREATE_HOOK!(FilterConsoleCommands, ACTIVE, NONE, (),
(callback: Option<ArchiveCallback>), {});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_hook_uses_normal_active_registration() {
        let hook = inventory::iter::<crate::resolvers::HookRegistration>
            .into_iter().find(|hook| hook.name == "FilterConsoleCommands").unwrap();
        assert!((hook.condition)());
    }

    #[test]
    fn registration_does_not_invoke_callback() {
        unsafe extern "C" fn callback(_: *mut c_void, _: *mut c_void) {
            panic!("Archive callback must remain unregistered");
        }
        let _: unsafe extern "C" fn(Option<ArchiveCallback>) = FilterConsoleCommands_detour_fkt;
        unsafe {
            FilterConsoleCommands_detour_fkt(Some(callback));
            FilterConsoleCommands_detour_fkt(None);
        }
    }
}
