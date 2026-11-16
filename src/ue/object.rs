use super::*;

bitflags::bitflags! {
    #[derive(Debug, Clone)]
    pub struct EObjectFlags: u32 {
        const RF_NoFlags = 0x0000;
        const RF_Public = 0x0001;
        const RF_Standalone = 0x0002;
        const RF_MarkAsNative = 0x0004;
        const RF_Transactional = 0x0008;
        const RF_ClassDefaultObject = 0x0010;
        const RF_ArchetypeObject = 0x0020;
        const RF_Transient = 0x0040;
        const RF_MarkAsRootSet = 0x0080;
        const RF_TagGarbageTemp = 0x0100;
        const RF_NeedInitialization = 0x0200;
        const RF_NeedLoad = 0x0400;
        const RF_KeepForCooker = 0x0800;
        const RF_NeedPostLoad = 0x1000;
        const RF_NeedPostLoadSubobjects = 0x2000;
        const RF_NewerVersionExists = 0x4000;
        const RF_BeginDestroyed = 0x8000;
        const RF_FinishDestroyed = 0x00010000;
        const RF_BeingRegenerated = 0x00020000;
        const RF_DefaultSubObject = 0x00040000;
        const RF_WasLoaded = 0x00080000;
        const RF_TextExportTransient = 0x00100000;
        const RF_LoadCompleted = 0x00200000;
        const RF_InheritableComponentTemplate = 0x00400000;
        const RF_DuplicateTransient = 0x00800000;
        const RF_StrongRefOnFrame = 0x01000000;
        const RF_NonPIEDuplicateTransient = 0x02000000;
        const RF_Dynamic = 0x04000000;
        const RF_WillBeLoaded = 0x08000000;
    }
}
bitflags::bitflags! {
    #[derive(Debug, Clone)]
    pub struct EFunctionFlags: u32 {
        const FUNC_None = 0x0000;
        const FUNC_Final = 0x0001;
        const FUNC_RequiredAPI = 0x0002;
        const FUNC_BlueprintAuthorityOnly = 0x0004;
        const FUNC_BlueprintCosmetic = 0x0008;
        const FUNC_Net = 0x0040;
        const FUNC_NetReliable = 0x0080;
        const FUNC_NetRequest = 0x0100;
        const FUNC_Exec = 0x0200;
        const FUNC_Native = 0x0400;
        const FUNC_Event = 0x0800;
        const FUNC_NetResponse = 0x1000;
        const FUNC_Static = 0x2000;
        const FUNC_NetMulticast = 0x4000;
        const FUNC_UbergraphFunction = 0x8000;
        const FUNC_MulticastDelegate = 0x00010000;
        const FUNC_Public = 0x00020000;
        const FUNC_Private = 0x00040000;
        const FUNC_Protected = 0x00080000;
        const FUNC_Delegate = 0x00100000;
        const FUNC_NetServer = 0x00200000;
        const FUNC_HasOutParms = 0x00400000;
        const FUNC_HasDefaults = 0x00800000;
        const FUNC_NetClient = 0x01000000;
        const FUNC_DLLImport = 0x02000000;
        const FUNC_BlueprintCallable = 0x04000000;
        const FUNC_BlueprintEvent = 0x08000000;
        const FUNC_BlueprintPure = 0x10000000;
        const FUNC_EditorOnly = 0x20000000;
        const FUNC_Const = 0x40000000;
        const FUNC_NetValidate = 0x80000000;
        const FUNC_AllFlags = 0xffffffff;
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct UObjectBase {
    vtable: *const UObjectVTable,
    pub object_flags: EObjectFlags,
    pub internal_index: i32,
    pub class_private: *const UClass,
    pub name_private: FName,
    pub outer_private: *const UObject,
}

#[derive(Debug)]
#[repr(C)]
pub struct UObjectBaseUtility {
    pub uobject_base: UObjectBase,
}

#[derive(Debug)]
#[repr(C)]
pub struct UObject {
    pub uobject_base_utility: UObjectBaseUtility,
}

#[derive(Debug)]
#[repr(C)]
struct UObjectVTable {
    _unused: [usize; 0x150 / 8],
    get_world: unsafe extern "C" fn(*const UObject) -> *mut crate::game::engine::UWorld,
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

fn class_chain_matches<K, S: AsRef<str>>(
    mut class: K,
    name: &str,
    mut inspect: impl FnMut(K) -> Option<(S, K)>,
) -> bool {
    // Bound corrupted or cyclic metadata; do not follow it indefinitely.
    for _ in 0..32 {
        let Some((class_name, parent)) = inspect(class) else { return false; };
        if class_name.as_ref() == name { return true; }
        class = parent;
    }
    false
}

impl UObject {
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
        if Self::is_live(object) { object.as_ref() } else { None }
    }

    pub fn name(&self) -> String {
        self.uobject_base_utility.uobject_base.name_private.to_string()
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
        if !Self::is_live(self) { return false; }
        let class = self.uobject_base_utility.uobject_base.class_private.cast::<UStruct>();
        class_chain_matches(class, name, |class| {
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

    /// # Safety
    /// This must be a live native object with the supported GetWorld vtable ABI.
    #[inline(always)]
    pub unsafe fn get_world(&self) -> *mut crate::game::engine::UWorld {
        ((*self.uobject_base_utility.uobject_base.vtable).get_world)(self)
    }
}

/// Partial native controller layout through Player; later fields are omitted.
#[repr(C)]
pub struct APlayerController {
    pub base: UObject,
    _padding_to_player: [u8; 0x2A8],
    pub player: *mut UPlayer, // 0x02D0
}

/// Native UPlayer prefix; sufficient for UObject class checks.
#[repr(C)]
pub struct UPlayer {
    pub base: UObject,
}

#[repr(C)]
pub struct ULocalPlayer {
    pub base: UPlayer,
}

// SAFETY: These repr(C) prefixes begin with UObject and map to their native classes.
unsafe impl UObjectType for UObject { const CLASS_NAME: &'static str = "Object"; }
unsafe impl UObjectType for APlayerController { const CLASS_NAME: &'static str = "PlayerController"; }
unsafe impl UObjectType for UPlayer { const CLASS_NAME: &'static str = "Player"; }
unsafe impl UObjectType for ULocalPlayer { const CLASS_NAME: &'static str = "LocalPlayer"; }

#[cfg(test)]
mod cast_tests {
    use super::*;

    #[test]
    fn display_names_strip_instance_and_blueprint_suffixes() {
        for (native, display) in [
            ("Sword_123", "Sword"),
            ("Sword_C_123", "Sword"),
            ("Sword_C", "Sword"),
            ("Sword_Archer", "Sword_Archer"),
            ("Sword", "Sword"),
            ("", ""),
        ] {
            assert_eq!(UObject::normalize_display_name(native), display);
        }
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
            assert!(class_chain_matches(2, name, |class| match class {
                2 => Some(("TBLPlayerState", 1)),
                1 => Some(("PlayerState", 0)),
                _ => None,
            }));
        }
        assert!(!class_chain_matches(1, "Character", |class| match class {
            1 => Some(("CharacterDeathcam_C", 0)),
            _ => None,
        }));
    }

    #[test]
    fn invalid_metadata_stops_traversal() {
        let mut inspected = Vec::new();
        assert!(!class_chain_matches(2, "PlayerState", |class| {
            inspected.push(class);
            if class == 2 { Some(("Unrelated", 1)) } else { None }
        }));
        assert_eq!(inspected, [2, 1]);
    }

    #[test]
    fn cyclic_metadata_is_bounded() {
        let mut inspected = 0;
        assert!(!class_chain_matches(1, "PlayerState", |class| {
            inspected += 1;
            Some(("Unrelated", class))
        }));
        assert!(inspected > 0 && inspected < 100);
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FOutputDevice {
    vtable: *const c_void,
    b_suppress_event_tag: bool,
    b_auto_emit_line_terminator: bool,
}

#[derive(Debug)]
#[repr(C)]
pub struct UField {
    pub uobject: UObject,
    pub next: *const UField,
}

#[derive(Debug)]
#[repr(C)]
pub struct FStructBaseChain {
    pub struct_base_chain_array: *const *const FStructBaseChain,
    pub num_struct_bases_in_chain_minus_one: i32,
}

#[derive(Debug)]
#[repr(C)]
pub struct FFieldClass {
    // TODO: remaining members
    /// Field type name, e.g. `StrProperty`, `SetProperty`.
    name: FName,
}

#[derive(Debug)]
#[repr(C)]
pub struct FFieldVariant {
    container: *const c_void,
    b_is_uobject: bool,
}

#[derive(Debug)]
#[repr(C)]
pub struct FField {
    vtable: *const c_void,
    pub class_private: *const FFieldClass,
    pub owner: FFieldVariant,
    pub next: *const FField,
    pub name_private: FName,
    pub flags_private: EObjectFlags,
}
const _: () = assert!(std::mem::offset_of!(FField, next) == 0x20);
const _: () = assert!(std::mem::offset_of!(FField, name_private) == 0x28);
const _: () = assert!(std::mem::size_of::<FField>() == 0x38);

/// Prefix of UE 4.25 `FProperty`; later members are not needed.
#[repr(C)]
pub struct FProperty {
    pub field: FField,
    pub array_dim: i32,
    pub element_size: i32,
    pub property_flags: u64,
    pub rep_index: u16,
    pub blueprint_replication_condition: u8,
    /// Byte offset of the value inside its owning object.
    pub offset_internal: i32,
}
const _: () = assert!(std::mem::offset_of!(FProperty, offset_internal) == 0x4C);

/// A property located by name on a class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundProperty {
    /// Byte offset of the value inside an instance.
    pub offset: usize,
    /// Field type name, e.g. `StrProperty`.
    pub type_name: String,
}

/// Looks up a property by name on `class` and its super classes, reading only
/// the class's own field list (no engine calls besides `FName` to string).
/// Returns `None` if there is no such property or its offset is out of range.
///
/// # Safety
/// `class` must be null or a live `UClass`.
pub unsafe fn find_property(class: *const UClass, name: &str) -> Option<FoundProperty> {
    // Bounds guard against a wrong layout turning into an endless walk.
    const MAX_FIELDS: usize = 4096;
    let mut current = class.cast::<UStruct>();
    let mut visited = 0usize;
    while !current.is_null() {
        let size = (*current).properties_size.max(0) as usize;
        let mut field = (*current).child_properties;
        while !field.is_null() {
            visited += 1;
            if visited > MAX_FIELDS {
                return None;
            }
            if (*field).name_private.to_string() == name {
                let offset = (*field.cast::<FProperty>()).offset_internal;
                if offset < 0 || offset as usize >= size {
                    return None;
                }
                let field_class = (*field).class_private;
                let type_name = if field_class.is_null() {
                    String::new()
                } else {
                    (*field_class).name.to_string()
                };
                return Some(FoundProperty { offset: offset as usize, type_name });
            }
            field = (*field).next;
        }
        current = (*current).super_struct;
    }
    None
}

#[derive(Debug)]
#[repr(C)]
pub struct UStruct {
    pub ufield: UField,
    pub fstruct_base_chain: FStructBaseChain,
    pub super_struct: *const UStruct,
    pub children: *const UField,
    pub child_properties: *const FField,
    pub properties_size: i32,
    pub min_alignment: i32,
    pub script: TArray<u8>,
    pub property_link: *const FProperty,
    pub ref_link: *const FProperty,
    pub destructor_link: *const FProperty,
    pub post_construct_link: *const FProperty,
    pub script_and_property_object_references: TArray<*const UObject>,
    pub unresolved_script_properties: *const (), //TODO pub TArray<TTuple<TFieldPath<FField>,int>,TSizedDefaultAllocator<32> >*
    pub unversioned_schema: *const (),           //TODO const FUnversionedStructSchema*
}

#[derive(Debug)]
#[repr(C)]
pub struct UFunction {
    pub ustruct: UStruct,
    pub function_flags: EFunctionFlags,
    pub num_parms: u8,
    pub parms_size: u16,
    pub return_value_offset: u16,
    pub rpc_id: u16,
    pub rpc_response_id: u16,
    pub first_property_to_init: *const FProperty,
    pub event_graph_function: *const UFunction,
    pub event_graph_call_offset: i32,
    pub func: unsafe extern "system" fn(*mut UObject, *mut kismet::FFrame, *mut c_void),
}

#[repr(C, packed(8))]
pub struct UBlueprintCore {
    pub base: UObject,                             // 0x0000 (0x28)
    pub skeleton_generated_class: *mut UClass,     // 0x0028 (0x08)
    pub generated_class: *mut UClass,              // 0x0030 (0x08)
    
    // 0x50 - 0x38 = 0x18 (24 bytes)
    pub padding_metadata: [u8; 0x18],              // 0x0038 (0x18)
}

#[repr(C)]
pub struct UBlueprint {
    pub base: UBlueprintCore,                      // 0x0000 (0x50)
    pub parent_class: *mut UClass,                 // 0x0050 (0x08)
    pub blueprint_type: u8,                        // 0x0058 (0x01)
    pub b_recompile_on_load: u8,                   // 0x0059 (0x01)
    pub b_has_been_regenerated: u8,                // 0x005A (0x01)
    pub b_is_regenerating_on_load: u8,             // 0x005B (0x01)
    pub blueprint_system_version: i32,             // 0x005C (0x04)
    pub simple_construction_script: *mut c_void,   // 0x0060 (0x08)
    pub component_templates: TArray<*mut c_void>,  // 0x0068 (0x10)
    pub timelines: TArray<*mut c_void>,            // 0x0078 (0x10)
    pub component_class_overrides: TArray<u8>,     // 0x0088 (0x10)
    pub inheritable_component_handler: *mut c_void,// 0x0098 (0x08)
}
// #[derive(Debug)]
// #[repr(C)]
// pub struct UClass {
//     pub ustruct: UStruct,
// }
// // ClassDefaultObject
#[repr(C)]
pub struct UClass {
    // pub _padding_base: [u8; 0xB0],                // 0x0000
    pub ustruct: UStruct,

    pub class_constructor: *const c_void,         // 0x00B0
    pub class_vtable_helper_ctor_caller: *const c_void, // 0x00B8
    pub class_add_referenced_objects: *const c_void,    // 0x00C0
    
    pub class_unique: u32,                        // 0x00C8
    pub class_flags: u32,                         // 0x00CC
    
    pub class_cast_flags: u64,                    // 0x00D0
    pub class_within: *mut c_void,                // 0x00D8
    pub class_generated_by: *mut c_void,          // 0x00E0
    pub class_config_name: FName,                 // 0x00E8 (8 bytes)

    pub class_reps: TArray<c_void>,               // 0x00F0 (16 bytes)
    
    // 0x0100:
    pub net_fields_ptr: *const c_void,            // 0x0100
    
    // 0x0108: 
    pub void_ptr_108: *const c_void,              // 0x0108
    
    // 0x0110:
    pub first_owned_class_rep_ptr: *const c_void, // 0x0110
    
    // 0x0118: 
    pub class_metadata_bits: u64,                 // 0x0118
    
    // 0x0120:
    pub class_default_object: *mut c_void,        // 0x0120 

    pub sparse_class_data: *mut c_void,           // 0x0128
    pub sparse_class_data_struct: *mut c_void,    // 0x0130
}


use std::ops::Deref;
use std::sync::atomic::Ordering;
use crate::globals;
use super::object_array::FUObjectArray;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FWeakObjectPtr {
    object_index: i32,
    object_serial_number: i32,
}
impl FWeakObjectPtr {
    pub fn new(object: &UObjectBase) -> Self {
        Self::new_from_index(object.internal_index)
    }
    pub fn new_from_index(index: i32) -> Self {
        Self {
            object_index: index,
            // serial allocation performs only atomic operations
            object_serial_number: unsafe {
                globals()
                    .guobject_array_unchecked()
                    .allocate_serial_number(index)
            },
        }
    }
    pub unsafe fn get<'a>(&self, object_array: &'a FUObjectArray) -> Option<&'a UObjectBase> {
        let guard = object_array.objects();
        let objects = guard.deref();
        if self.object_index < 0 || self.object_index >= objects.num_elements {
            return None;
        }
        let item = objects.item(self.object_index);
        if item.serial_number.load(Ordering::Relaxed) != self.object_serial_number {
            return None;
        }
        item.object.as_ref()
    }
}

impl UObjectBase {
    pub unsafe fn get_path_name(&self, stop_outer: Option<&UObject>) -> String {
        let mut string = FString::new();
        unsafe {
            globals().uobject_base_utility_get_path_name()(self, stop_outer, &mut string);
        }
        string.to_string()
    }
}
