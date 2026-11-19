//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/Object.h>.

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::UClass;
use crate::chivalry2::engine::UFunction;
use crate::chivalry2::engine::UStruct;
use crate::chivalry2::engine::UWorld;
use crate::chivalry2::engine::TCHAR;
use crate::chivalry2::engine::*;
use crate::features::config_overrides::{
    resolve_profile_override_paths_for_config, LOAD_CONFIG_REENTRY,
};
use crate::globals;
use crate::tools::hook_globals::cli_args;
use crate::tools::hook_globals::globals_initialized;
use std::ffi::c_void;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
};
use widestring::U16CString;

define_pattern_resolver!(LoadConfig, [
    "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 D0 FC FF FF 48 81 EC 30 04 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 10 03 00 00"
]);

define_pattern_resolver!(
    ProcessEvent,
    ["40 55 56 57 41 54 41 55 41 56 41 57 48 81 EC F0 00 00 00 48 8D 6C 24 30 48 89 9D 18 01"]
);

#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct UObject {
    #[inherit]
    pub uobject_base_utility: UObjectBaseUtility,
}

#[unchained_macros::runtime_bindings]
impl UObject {
    pub unsafe fn get_uobject_from_path(path: &str, load_if_missing: bool) -> *mut UObject {
        let wide_path = U16CString::from_str(path).unwrap();

        if !load_if_missing {
            // Try to find it in memory first
            return crate::chivalry2::engine::static_find_object(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                wide_path.as_ptr(),
                false,
            );
        }

        crate::chivalry2::engine::static_load_object(
            std::ptr::null_mut(), // Any class
            std::ptr::null_mut(), // No specific outer
            wide_path.as_ptr(),
            std::ptr::null(),     // Filename (usually null)
            0,                    // LoadFlags (0 = LoadConfig)
            std::ptr::null_mut(), // Sandbox
            false,                // AllowNativeComponentClass
            std::ptr::null_mut(),
        )
    }

    /// Validate a raw pointer against GUObjectArray without first dereferencing it.
    ///
    /// # Safety
    /// Engine globals must be initialized. Call on the game thread while object
    /// destruction/GC cannot race this check; this does not pin the object.
    pub unsafe fn is_live(object: *const Self) -> bool {
        !object.is_null() && globals().guobject_array_unchecked().is_live(object.cast())
    }

    /// Borrow a currently registered native object after a liveness check.
    ///
    /// # Safety
    /// The is_live contract applies. The object must remain alive and shared
    /// access must remain valid for the returned lifetime; validation does not
    /// establish ownership, detect address reuse or prevent subsequent destruction.
    pub unsafe fn from_ptr<'a>(object: *const Self) -> Option<&'a Self> {
        if Self::is_live(object) {
            object.as_ref()
        } else {
            None
        }
    }

    pub fn name(&self) -> String {
        self.uobject_base_utility
            .uobject_base
            .name_private
            .to_string()
    }

    pub fn display_name(&self) -> String {
        Self::normalize_display_name(&self.name())
    }

    fn normalize_display_name(name: &str) -> String {
        let name = match name.rsplit_once('_') {
            Some((base, suffix)) if suffix.chars().all(|c| c.is_ascii_digit()) => base,
            _ => name,
        };
        name.trim_end_matches("_C").to_owned()
    }

    /// # Safety
    /// The from_ptr contract applies to this object and its class metadata.
    pub unsafe fn class_name(&self) -> Option<String> {
        let class = self.uobject_base_utility.uobject_base.class_private;
        Some(Self::from_ptr(class.cast())?.name())
    }

    /// Match an exact class name anywhere in the validated native superclass chain.
    ///
    /// # Safety
    /// The from_ptr contract applies to this object and its native class metadata.
    pub unsafe fn inherits_from(&self, name: &str) -> bool {
        if !Self::is_live(self) {
            return false;
        }
        let class = self
            .uobject_base_utility
            .uobject_base
            .class_private
            .cast::<UStruct>();
        UObject::class_chain_matches(class, name, |class| {
            let object = Self::from_ptr(class.cast())?;
            Some((object.name(), (*class).super_struct))
        })
    }

    /// Borrow a native view only when its mapped class matches this object.
    ///
    /// # Safety
    /// The from_ptr contract applies to this object and its native class metadata.
    /// The engine build must match T's declared layout. No ownership is transferred.
    pub unsafe fn uobj_cast<T: UObjectType>(&self) -> Option<&T> {
        if self.inherits_from(T::CLASS_NAME) {
            (self as *const Self).cast::<T>().as_ref()
        } else {
            None
        }
    }

    fn class_chain_matches<K, S: AsRef<str>>(
        mut class: K,
        name: &str,
        mut inspect: impl FnMut(K) -> Option<(S, K)>,
    ) -> bool {
        // Bound corrupted or cyclic metadata; do not follow it indefinitely.
        for _ in 0..32 {
            let Some((class_name, parent)) = inspect(class) else {
                return false;
            };
            if class_name.as_ref() == name {
                return true;
            }
            class = parent;
        }
        false
    }

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
            let path = unsafe { &*self }.uobject_base_utility.get_path_name(None);
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
            let current_class = unsafe {
                (*object).uobject_base_utility.uobject_base.class_private as *const UStruct
            };

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

        let index = UObject::process_event_function_index(function);

        // handle known problematic functions
        if UObject::process_event_is_blocked(function, index) {
            let should_log =
                PROCESS_EVENT_BLOCK_LOGGED.with(|logged| logged.borrow_mut().insert(index));

            if should_log {
                let (_, func_name) = UObject::process_event_names(std::ptr::null_mut(), function);

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
                let (class_name, func_name) = UObject::process_event_names(object, function);

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

/// Maps a Rust native-object prefix to its exact UE class name.
///
/// # Safety
/// Implementors must have the supported native layout, alignment and UObject
/// prefix at offset zero for this class and its subclasses. Never implement for
/// an opaque alias such as c_void.
pub unsafe trait UObjectType: Sized {
    const CLASS_NAME: &'static str;

    /// Validate a raw native pointer and borrow this class's view.
    ///
    /// # Safety
    /// UObject::from_ptr's engine-thread and lifetime requirements apply, and
    /// the engine build must match this type's native layout.
    #[inline(always)]
    unsafe fn from_ptr<'a, P>(object: *const P) -> Option<&'a Self> {
        UObject::from_ptr(object.cast())?.uobj_cast::<Self>()
    }
}

// SAFETY: These repr(C) prefixes begin with UObject and map to their native classes.
unsafe impl UObjectType for UObject {
    const CLASS_NAME: &'static str = "Object";
}

#[cfg(test)]
mod cast_tests {
    use super::*;

    #[test_case::test_case("Sword_123", "Sword"; "instance suffix")]
    #[test_case::test_case("Sword_C_123", "Sword"; "blueprint instance suffix")]
    #[test_case::test_case("Sword_C", "Sword"; "blueprint suffix")]
    #[test_case::test_case("Sword_Archer", "Sword_Archer"; "nonnumeric suffix")]
    #[test_case::test_case("Sword", "Sword"; "plain name")]
    #[test_case::test_case("", ""; "empty name")]
    fn display_names_strip_instance_and_blueprint_suffixes(native: &str, display: &str) {
        assert_eq!(UObject::normalize_display_name(native), display);
    }

    #[test]
    fn null_object_is_rejected_before_accessing_engine_globals() {
        assert!(unsafe { UObject::from_ptr(std::ptr::null()) }.is_none());
    }

    #[test]
    fn typed_null_object_is_rejected_before_accessing_engine_globals() {
        assert!(unsafe { ULocalPlayer::from_ptr(std::ptr::null::<UPlayer>()) }.is_none());
    }

    #[test]
    fn matches_exact_class_or_ancestor_not_substrings() {
        for name in ["TBLPlayerState", "PlayerState"] {
            assert!(UObject::class_chain_matches(2, name, |class| match class {
                2 => Some(("TBLPlayerState", 1)),
                1 => Some(("PlayerState", 0)),
                _ => None,
            }));
        }
        assert!(!UObject::class_chain_matches(
            1,
            "Character",
            |class| match class {
                1 => Some(("CharacterDeathcam_C", 0)),
                _ => None,
            }
        ));
    }

    #[test]
    fn invalid_metadata_stops_traversal() {
        let mut inspected = Vec::new();
        assert!(!UObject::class_chain_matches(2, "PlayerState", |class| {
            inspected.push(class);
            if class == 2 {
                Some(("Unrelated", 1))
            } else {
                None
            }
        }));
        assert_eq!(inspected, [2, 1]);
    }

    #[test]
    fn cyclic_metadata_is_bounded() {
        let mut inspected = 0;
        assert!(!UObject::class_chain_matches(1, "PlayerState", |class| {
            inspected += 1;
            Some(("Unrelated", class))
        }));
        assert!(inspected > 0 && inspected < 100);
    }
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

/// Upcast a native object pointer without dereferencing it or changing mutability.
pub trait AsUObject {
    type Pointer;

    fn as_uobject(self) -> Self::Pointer;
}

// These upcasts rely on the embedded UObject being at offset zero.
const _: () = assert!(std::mem::offset_of!(UWorld, base) == 0);

impl AsUObject for *mut UWorld {
    type Pointer = *mut UObject;

    fn as_uobject(self) -> Self::Pointer {
        self.cast()
    }
}

impl AsUObject for *const UWorld {
    type Pointer = *const UObject;

    fn as_uobject(self) -> Self::Pointer {
        self.cast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upcasts_preserve_null_and_mutability() {
        let mutable: *mut UObject = std::ptr::null_mut::<UWorld>().as_uobject();
        let immutable: *const UObject = std::ptr::null::<UWorld>().as_uobject();
        assert!(mutable.is_null());
        assert!(immutable.is_null());
    }

    #[test]
    fn upcasts_preserve_address_without_reading_world() {
        let mut storage = std::mem::MaybeUninit::<UWorld>::uninit();
        let mutable = storage.as_mut_ptr();
        let immutable = storage.as_ptr();
        assert_eq!(mutable.as_uobject().cast::<UWorld>(), mutable);
        assert_eq!(immutable.as_uobject().cast::<UWorld>(), immutable);
    }
}
