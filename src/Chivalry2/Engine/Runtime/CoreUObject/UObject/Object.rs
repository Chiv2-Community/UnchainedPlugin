//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/Object.h>.

use crate::Chivalry2::Engine::UClass;
use crate::Chivalry2::Engine::UFunction;
use crate::Chivalry2::Engine::UStruct;
use crate::Chivalry2::Engine::UWorld;
use crate::Chivalry2::Engine::TCHAR;
use crate::Chivalry2::Engine::*;
use crate::globals;
use std::ffi::c_void;
use widestring::U16CString;

define_pattern_resolver!(UObject__LoadConfig, [
    "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 D0 FC FF FF 48 81 EC 30 04 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 10 03 00 00"
]);

define_pattern_resolver!(
    UObject__ProcessEvent,
    ["40 55 56 57 41 54 41 55 41 56 41 57 48 81 EC F0 00 00 00 48 8D 6C 24 30 48 89 9D 18 01"]
);

#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct UObject {
    #[inherit]
    #[vtable(
        offset(0x150);
        GetWorld(&self) -> *mut UWorld;
    )]
    pub uobject_base_utility: UObjectBaseUtility,
}

#[unchained_macros::runtime_bindings]
impl UObject {
    pub unsafe fn get_uobject_from_path(path: &str, load_if_missing: bool) -> *mut UObject {
        let wide_path = U16CString::from_str(path).unwrap();

        if !load_if_missing {
            // Try to find it in memory first
            return crate::Chivalry2::Engine::StaticFindObject(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                wide_path.as_ptr(),
                false,
            );
        }

        crate::Chivalry2::Engine::StaticLoadObject(
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

    #[unchained_macros::bind_resolved_func(UObject__LoadConfig)]
    pub(crate) unsafe fn LoadConfig(
        self: *mut Self,
        config_class: *mut UClass,
        filename: *mut TCHAR,
        propagation_flags: u32,
        property_to_load: *mut c_void,
    ) {
    }

    #[unchained_macros::bind_resolved_func(UObject__ProcessEvent)]
    pub(crate) unsafe fn ProcessEvent(
        self: *mut Self,
        function: *mut UFunction,
        params: *mut c_void,
    ) {
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
    fn get_world_uses_uobject_slot_without_changing_the_native_prefix() {
        #[repr(C)]
        struct Fixture {
            vtable: *const *const (),
            flags: u32,
            index: i32,
            class: *const UClass,
            name: FName,
            outer: *const UObject,
        }

        unsafe extern "C" fn get_world(this: *const UObject) -> *mut UWorld {
            (*this).uobject_base_utility.uobject_base.outer_private as *mut UWorld
        }

        assert_eq!(std::mem::size_of::<Fixture>(), std::mem::size_of::<UObject>());
        assert_eq!(std::mem::size_of::<UObject>(), 0x28);
        assert!(!UObjectBase::__ctables_primary_has_slot(0x150));
        assert!(UObject::__ctables_primary_has_slot(0x150));
        let mut table = [std::ptr::null(); 0x150 / 8 + 1];
        table[0x150 / 8] = get_world as *const ();
        let mut sentinel = 0u8;
        let expected = (&raw mut sentinel).cast::<UWorld>();
        let fixture = Fixture {
            vtable: table.as_ptr(),
            flags: 0,
            index: 0,
            class: std::ptr::null(),
            name: FName::default(),
            outer: expected.cast(),
        };
        let object = unsafe { &*(&raw const fixture).cast::<UObject>() };
        assert_eq!(unsafe { object.GetWorld() }, expected);
    }

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
