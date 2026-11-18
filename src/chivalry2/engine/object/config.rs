//! UE 4.25-plus: [UObject::LoadConfig](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Private/UObject/Obj.cpp).

use crate::tools::hook_globals::cli_args;
use crate::tools::hook_globals::globals_initialized;
use crate::chivalry2::engine::{FString, UClass, UObject, TCHAR};
use std::ffi::c_void;
mod overrides;
use overrides::{resolve_profile_override_paths_for_config, LOAD_CONFIG_REENTRY};

define_pattern_resolver!(LoadConfig, [
    "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 D0 FC FF FF 48 81 EC 30 04 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 10 03 00 00"
]);

#[unchained_macros::runtime_bindings]
impl UObject {
    #[bind_resolved_func_detour(LoadConfig, || cli_args().unchained_profile_location.is_some())]
    unsafe fn load_config_detour(
        self: *mut Self,
        config_class: *mut UClass,
        filename: *mut TCHAR,
        propagation_flags: u32,
        property_to_load: *mut c_void,
    ) {
        if LOAD_CONFIG_REENTRY.with(|state| state.get()) || !globals_initialized() {
            return unsafe {
                self.load_config(config_class, filename, propagation_flags, property_to_load)
            };
        }
        let object = unsafe { &*self };
        let actual_class = object.uobject_base_utility.uobject_base.class_private as *mut UClass;
        // GetConfigName is fatal for classes without CLASS_Config. Mirror the
        // native early return before looking up the base filename.
        // https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Private/UObject/Obj.cpp#L2017-L2020
        const CLASS_CONFIG: u32 = 0x0000_0004;
        if actual_class.is_null() || unsafe { (*actual_class).class_flags } & CLASS_CONFIG == 0 {
            return unsafe {
                self.load_config(config_class, filename, propagation_flags, property_to_load)
            };
        }
        let config_name = {
            let mut storage = FString::default();
            let result = unsafe { actual_class.get_config_name(&mut storage) };
            if result.is_null() {
                if storage.is_empty() {
                    None
                } else {
                    Some(storage.to_string())
                }
            } else {
                Some(unsafe { (*result).to_string() })
            }
        };
        let original_filename = if filename.is_null() {
            config_name
        } else {
            let parsed = unsafe { widestring::U16CStr::from_ptr_str(filename) }.to_string_lossy();
            if parsed.trim().is_empty() {
                config_name
            } else {
                Some(parsed)
            }
        };
        let object_path = if self.is_null() {
            None
        } else {
            let path = unsafe { &*self }
                .uobject_base_utility
                .uobject_base
                .get_path_name(None);
            if path.trim().is_empty() {
                None
            } else {
                Some(path)
            }
        };
        let filenames = resolve_profile_override_paths_for_config(
            original_filename.as_deref(),
            object_path.as_deref(),
        );
        if !filenames.is_empty() {
            LOAD_CONFIG_REENTRY.with(|state| state.set(true));
            unsafe {
                self.load_config(config_class, filename, propagation_flags, property_to_load);
            }
            for name in &filenames {
                let path = match widestring::U16CString::from_str(name.as_str()) {
                    Ok(path) => path,
                    Err(_error) => {
                        #[cfg(feature = "config_overrides_verbose_hooks")]
                        swarn!("Skipping invalid override path {name}: {_error}");
                        continue;
                    }
                };
                unsafe {
                    self.load_config(
                        config_class,
                        path.as_ptr() as *mut TCHAR,
                        propagation_flags,
                        property_to_load,
                    );
                }
            }
            LOAD_CONFIG_REENTRY.with(|state| state.set(false));
            return;
        }
        unsafe {
            self.load_config(config_class, filename, propagation_flags, property_to_load);
        }
    }
}
