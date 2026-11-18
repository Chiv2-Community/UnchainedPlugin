//! UE 4.25-plus: [UObject::ProcessEvent](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Private/UObject/ScriptCore.cpp).

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    os::raw::c_void,
};

use crate::{
    tools::hook_globals::cli_args,
    ue::{UFunction, UObject, UStruct},
};

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

define_pattern_resolver!(
    ProcessEvent,
    ["40 55 56 57 41 54 41 55 41 56 41 57 48 81 EC F0 00 00 00 48 8D 6C 24 30 48 89 9D 18 01"]
);

#[unchained_macros::runtime_bindings]
impl UObject {
    #[bind_resolved_func_detour(ProcessEvent, || cli_args().apply_desync_patch)]
    pub(crate) unsafe fn process_event_detour(
        self: *mut Self,
        function: *mut UFunction,
        params: *mut c_void,
    ) {
        let object = self;
        if function.is_null() {
            return unsafe { self.process_event(function, params) };
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

        unsafe { self.process_event(function, params) }
    }
}
