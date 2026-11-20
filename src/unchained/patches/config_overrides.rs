//! Profile-specific configuration loading.

use crate::chivalry2::engine::{EClassFlags, FString, UClass, UObject, TCHAR};
use crate::features::config_overrides::{
    resolve_profile_override_paths_for_config, LOAD_CONFIG_REENTRY,
};
use crate::tools::hook_globals::{cli_args, globals_initialized};
use std::ffi::c_void;

#[unchained_macros::bind_func_detour(UObject::LoadConfig, || cli_args().unchained_profile_location.is_some())]
pub(crate) unsafe fn load_config_detour(
    this: *mut UObject,
    config_class: *mut UClass,
    filename: *mut TCHAR,
    propagation_flags: u32,
    property_to_load: *mut c_void,
) {
    if LOAD_CONFIG_REENTRY.with(|state| state.get()) || !globals_initialized() {
        return unsafe {
            this.LoadConfig(config_class, filename, propagation_flags, property_to_load)
        };
    }
    let object = unsafe { &*this };
    let actual_class = object.uobject_base_utility.uobject_base.class_private as *mut UClass;
    // GetConfigName is fatal for classes without CLASS_Config. Mirror the
    // native early return before looking up the base filename.
    // https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Private/UObject/Obj.cpp#L2017-L2020
    if actual_class.is_null()
        || unsafe { (*actual_class).class_flags } & EClassFlags::CLASS_Config.bits() == 0
    {
        return unsafe {
            this.LoadConfig(config_class, filename, propagation_flags, property_to_load)
        };
    }
    let config_name = {
        let mut storage = FString::default();
        let result = unsafe { actual_class.GetConfigName(&mut storage) };
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
    let object_path = if this.is_null() {
        None
    } else {
        let path = unsafe { &*this }.uobject_base_utility.GetPathName(None);
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
            this.LoadConfig(config_class, filename, propagation_flags, property_to_load);
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
                this.LoadConfig(
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
        this.LoadConfig(config_class, filename, propagation_flags, property_to_load);
    }
}

#[unchained_macros::bind_func_detour(UClass::GetConfigName, || crate::tools::hook_globals::cli_args().unchained_profile_location.is_some())]
pub(crate) unsafe fn get_config_name_detour(
    this: *mut UClass,
    storage: *mut crate::chivalry2::engine::FString,
) -> *mut crate::chivalry2::engine::FString {
    unsafe { this.GetConfigName(storage) }
}
