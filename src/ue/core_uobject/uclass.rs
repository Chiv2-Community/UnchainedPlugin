use crate::ue::{FName, TArray, UObject, UStruct};
use std::ffi::c_void;
use unchained_macros::bind_resolver_func;

define_pattern_resolver!(GetDefaultObject,["40 53 48 83 EC 20 48 8B 81 18 01 00 00 48 8B D9 48 85 C0 ?? ?? 84 D2 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ??"]);

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

impl UClass {
    /// UClass::GetDefaultObject(bool) const; may lazily create the CDO.
    ///
    /// # Safety
    /// Self must be a live native class; call on the engine thread.
    #[bind_resolver_func(self::GetDefaultObject)]
    pub unsafe fn get_default_object(self: *const Self, create_if_needed: bool) -> *mut UObject {}
}
