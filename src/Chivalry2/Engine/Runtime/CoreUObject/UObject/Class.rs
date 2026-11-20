//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/Class.h>.

use crate::Chivalry2::Engine::FName;
use crate::Chivalry2::Engine::TArray;
use crate::Chivalry2::Engine::UObject;
use crate::Chivalry2::Engine::*;
use std::ffi::c_void;

define_pattern_resolver!(GetConfigName, [
    "48 89 5C 24 10 48 89 74 24 18 57 48 83 EC 70 48 8D B9 E8 00 00 00 48 8B DA 48 8B F1 BA 1F 00 00 00 8B 0F E8 ?? ?? ?? ?? 83 7F 04 00 41 0F 94 C0 44 84 C0 74 15 33 C0 48 89 03 48 8B 3D ?? ?? ?? ?? 48 8B"
]);

define_pattern_resolver!(GetDefaultObject,["40 53 48 83 EC 20 48 8B 81 18 01 00 00 48 8B D9 48 85 C0 ?? ?? 84 D2 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ??"]);

#[repr(C)]
pub struct UClass {
    // pub _padding_base: [u8; 0xB0],                // 0x0000
    pub ustruct: UStruct,

    pub class_constructor: *const c_void,               // 0x00B0
    pub class_vtable_helper_ctor_caller: *const c_void, // 0x00B8
    pub class_add_referenced_objects: *const c_void,    // 0x00C0

    pub class_unique: u32, // 0x00C8
    pub class_flags: u32,  // 0x00CC

    pub class_cast_flags: u64,           // 0x00D0
    pub class_within: *mut c_void,       // 0x00D8
    pub class_generated_by: *mut c_void, // 0x00E0
    pub class_config_name: FName,        // 0x00E8 (8 bytes)

    pub class_reps: TArray<c_void>, // 0x00F0 (16 bytes)

    // 0x0100:
    pub net_fields_ptr: *const c_void, // 0x0100

    // 0x0108:
    pub void_ptr_108: *const c_void, // 0x0108

    // 0x0110:
    pub first_owned_class_rep_ptr: *const c_void, // 0x0110

    // 0x0118:
    pub class_metadata_bits: u64, // 0x0118

    // 0x0120:
    pub class_default_object: *mut c_void, // 0x0120

    pub sparse_class_data: *mut c_void,        // 0x0128
    pub sparse_class_data_struct: *mut c_void, // 0x0130
}

#[unchained_macros::runtime_bindings]
impl UClass {
    #[unchained_macros::bind_resolved_func(GetConfigName)]
    pub(crate) unsafe fn GetConfigName(
        self: *mut Self,
        storage: *mut crate::Chivalry2::Engine::FString,
    ) -> *mut crate::Chivalry2::Engine::FString {
    }

    /// May lazily create the class default object.
    ///
    /// # Safety
    /// Self must be a live native class; call on the engine thread.
    #[bind_resolved_func(self::GetDefaultObject)]
    pub unsafe fn GetDefaultObject(self: *const Self, create_if_needed: bool) -> *mut UObject {}

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
                    return Some(FoundProperty {
                        offset: offset as usize,
                        type_name,
                    });
                }
                field = (*field).next;
            }
            current = (*current).super_struct;
        }
        None
    }
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

/// A property located by name on a class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundProperty {
    /// Byte offset of the value inside an instance.
    pub offset: usize,
    /// Field type name, e.g. `StrProperty`.
    pub type_name: String,
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

impl UStruct {
    /// Property field by name on `start` or its supers, as the raw `FField` pointer
    /// (unlike [`crate::Chivalry2::Engine::UClass::find_property`], which returns only offset + type name).
    ///
    /// # Safety
    /// `start` must be null or a live `UStruct` (a `UClass` cast to `UStruct`).
    pub(crate) unsafe fn find_field(start: *const UStruct, name: &str) -> Option<*const FField> {
        const MAX_FIELDS: usize = 4096;
        let mut current = start;
        let mut visited = 0usize;
        while !current.is_null() {
            let mut field = (*current).child_properties;
            while !field.is_null() {
                visited += 1;
                if visited > MAX_FIELDS {
                    return None;
                }
                if (*field).name_private.to_string() == name {
                    return Some(field);
                }
                field = (*field).next;
            }
            current = (*current).super_struct;
        }
        None
    }

    /// Member byte offsets inside a `UserDefinedStruct`, keyed by authored field name
    /// (the token before the first `_` in each member's mangled FName, e.g.
    /// `MapName_11_<GUID>` -> `MapName`).
    ///
    /// # Safety
    /// `strukt` must be null or a live `UScriptStruct` (read as its `UStruct` prefix).
    pub(crate) unsafe fn struct_member_offsets(
        strukt: *const UStruct,
    ) -> std::collections::HashMap<String, usize> {
        const MAX_FIELDS: usize = 256;
        let mut out = std::collections::HashMap::new();
        if strukt.is_null() {
            return out;
        }
        let mut field = (*strukt).child_properties;
        let mut visited = 0usize;
        while !field.is_null() {
            visited += 1;
            if visited > MAX_FIELDS {
                break;
            }
            let raw = (*field).name_private.to_string();
            let token = raw.split('_').next().unwrap_or(&raw).to_string();
            let offset = (*field.cast::<FProperty>()).offset_internal;
            if offset >= 0 {
                out.entry(token).or_insert(offset as usize);
            }
            field = (*field).next;
        }
        out
    }
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
    pub func: unsafe extern "system" fn(
        *mut UObject,
        *mut crate::Chivalry2::Engine::Runtime::CoreUObject::UObject::Stack::FFrame,
        *mut c_void,
    ),
}
