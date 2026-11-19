//! Plugin replacements for `crate::chivalry2::engine::runtime::core_uobject::uobject::object`.

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::UClass;
use crate::chivalry2::engine::UFunction;
use crate::chivalry2::engine::UStruct;
use crate::chivalry2::engine::TCHAR;
use crate::chivalry2::engine::*;
use crate::features::config_overrides::{
    resolve_profile_override_paths_for_config, LOAD_CONFIG_REENTRY,
};
use crate::tools::hook_globals::cli_args;
use crate::tools::hook_globals::globals_initialized;
use std::ffi::c_void;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
};

#[unchained_macros::bind_func_detour(UObject::load_config, || cli_args().unchained_profile_location.is_some())]
pub(crate) unsafe fn load_config_detour(
    this: *mut UObject,
    config_class: *mut UClass,
    filename: *mut TCHAR,
    propagation_flags: u32,
    property_to_load: *mut c_void,
) {
    if LOAD_CONFIG_REENTRY.with(|state| state.get()) || !globals_initialized() {
        return unsafe {
            this.load_config(config_class, filename, propagation_flags, property_to_load)
        };
    }
    let object = unsafe { &*this };
    let actual_class = object.uobject_base_utility.uobject_base.class_private as *mut UClass;
    // GetConfigName is fatal for classes without CLASS_Config. Mirror the
    // native early return before looking up the base filename.
    // https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Private/UObject/Obj.cpp#L2017-L2020
    const CLASS_CONFIG: u32 = 0x0000_0004;
    if actual_class.is_null() || unsafe { (*actual_class).class_flags } & CLASS_CONFIG == 0 {
        return unsafe {
            this.load_config(config_class, filename, propagation_flags, property_to_load)
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
    let object_path = if this.is_null() {
        None
    } else {
        let path = unsafe { &*this }.uobject_base_utility.get_path_name(None);
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
            this.load_config(config_class, filename, propagation_flags, property_to_load);
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
                this.load_config(
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
        this.load_config(config_class, filename, propagation_flags, property_to_load);
    }
}

#[inline]
fn process_event_function_index(function: *mut UFunction) -> u32 {
    unsafe {
        (*function)
            .ustruct
            .ufield
            .uobject
            .uobject_base_utility
            .uobject_base
            .name_private
            .comparison_index
            .value
    }
}

fn process_event_names(object: *mut UObject, function: *mut UFunction) -> (String, String) {
    let func_name = unsafe {
        (*function)
            .ustruct
            .ufield
            .uobject
            .uobject_base_utility
            .uobject_base
            .name_private
            .to_string()
    };

    let mut class_name = String::new();

    if !object.is_null() {
        let current_class =
            unsafe { (*object).uobject_base_utility.uobject_base.class_private as *const UStruct };

        if !current_class.is_null() {
            class_name = unsafe {
                (*current_class)
                    .ufield
                    .uobject
                    .uobject_base_utility
                    .uobject_base
                    .name_private
                    .to_string()
            };
        }
    }

    (class_name, func_name)
}

#[inline]
fn process_event_is_blocked(function: *mut UFunction, index: u32) -> bool {
    PROCESS_EVENT_BLOCK_CACHE.with(|cache| {
        if let Some(&blocked) = cache.borrow().get(&index) {
            return blocked;
        }

        // Only convert FName to a String on cache miss
        let func_name = unsafe {
            (*function)
                .ustruct
                .ufield
                .uobject
                .uobject_base_utility
                .uobject_base
                .name_private
                .to_string()
        };

        let blocked = BLOCKED_PROCESS_EVENT_NAMES.contains(&func_name.as_str());

        cache.borrow_mut().insert(index, blocked);
        blocked
    })
}

#[unchained_macros::bind_func_detour(UObject::process_event, || cli_args().apply_desync_patch)]
pub(crate) unsafe fn process_event_detour(
    this: *mut UObject,
    function: *mut UFunction,
    params: *mut c_void,
) {
    let object = this;
    if function.is_null() {
        return unsafe { this.process_event(function, params) };
    }

    let index = process_event_function_index(function);

    // handle known problematic functions
    if process_event_is_blocked(function, index) {
        let should_log =
            PROCESS_EVENT_BLOCK_LOGGED.with(|logged| logged.borrow_mut().insert(index));

        if should_log {
            let (_, func_name) = process_event_names(std::ptr::null_mut(), function);

            swarn!("Blocked ProcessEvent: {}", func_name);
        }

        return;
    }

    // recursion depth check
    let depth = PROCESS_EVENT_DEPTH.with(|d| d.get());

    if depth >= PROCESS_EVENT_MAX_DEPTH {
        let should_log = PROCESS_EVENT_DEPTH_LOGGED.with(|logged| {
            if logged.get() {
                false
            } else {
                logged.set(true);
                true
            }
        });

        // log this once (per thread)
        if should_log {
            let (class_name, func_name) = process_event_names(object, function);

            serror!(
                "Depth limit {} reached at {}::{}; dropping event",
                PROCESS_EVENT_MAX_DEPTH,
                class_name,
                func_name
            );
        }

        return;
    }

    // track nested
    PROCESS_EVENT_DEPTH.with(|d| d.set(depth + 1));
    let _depth_guard = ProcessEventDepthGuard;

    unsafe { this.process_event(function, params) }
}

// PROCESSEVENT FILTERING

// Always block these functions in PRocessEvent
const BLOCKED_PROCESS_EVENT_NAMES: &[&str] = &["ClientSetCameraMode"];

// Limit recursion depth
const PROCESS_EVENT_MAX_DEPTH: u32 = 120; // TODO: Nihi: check if this has false positives

thread_local! {
    // Current nested ProcessEvent depth (this thread)
    static PROCESS_EVENT_DEPTH: Cell<u32> = const { Cell::new(0) };

    // FName comparison index -> is function blocked?
    static PROCESS_EVENT_BLOCK_CACHE: RefCell<HashMap<u32, bool>> =
        RefCell::new(HashMap::new());

    // avoid logging every invocation
    static PROCESS_EVENT_BLOCK_LOGGED: RefCell<HashSet<u32>> =
        RefCell::new(HashSet::new());

    // Avoid repeated recursion warnings
    static PROCESS_EVENT_DEPTH_LOGGED: Cell<bool> = const { Cell::new(false) };
}

struct ProcessEventDepthGuard;

impl Drop for ProcessEventDepthGuard {
    fn drop(&mut self) {
        PROCESS_EVENT_DEPTH.with(|depth| {
            let next = depth.get().saturating_sub(1);
            depth.set(next);

            if next < PROCESS_EVENT_MAX_DEPTH {
                PROCESS_EVENT_DEPTH_LOGGED.with(|logged| {
                    logged.set(false);
                });
            }
        });
    }
}
