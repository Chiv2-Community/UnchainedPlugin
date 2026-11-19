//! UE 4.25-plus: [UClass](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/Class.h).

use crate::chivalry2::engine::{FName, TArray, UObject, UStruct};
use std::ffi::c_void;
use unchained_macros::bind_resolved_func;

define_pattern_resolver!(GetConfigName, [
    "48 89 5C 24 10 48 89 74 24 18 57 48 83 EC 70 48 8D B9 E8 00 00 00 48 8B DA 48 8B F1 BA 1F 00 00 00 8B 0F E8 ?? ?? ?? ?? 83 7F 04 00 41 0F 94 C0 44 84 C0 74 15 33 C0 48 89 03 48 8B 3D ?? ?? ?? ?? 48 8B"
]);

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

#[unchained_macros::runtime_bindings]
impl UClass {
    #[bind_resolved_func_detour(GetConfigName, || crate::tools::hook_globals::cli_args().unchained_profile_location.is_some())]
    pub(crate) unsafe fn get_config_name_detour(self: *mut Self, storage: *mut crate::chivalry2::engine::FString) -> *mut crate::chivalry2::engine::FString {
        unsafe { self.get_config_name(storage) }
    }

    /// UClass::GetDefaultObject(bool) const; may lazily create the CDO.
    ///
    /// # Safety
    /// Self must be a live native class; call on the engine thread.
    #[bind_resolved_func(self::GetDefaultObject)]
    pub unsafe fn get_default_object(self: *const Self, create_if_needed: bool) -> *mut UObject {}
}
