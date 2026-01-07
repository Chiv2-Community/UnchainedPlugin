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
    pub vtable: *const c_void,
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
    // TODO
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
    pub class_private: *const FFieldClass,
    pub owner: FFieldVariant,
    pub next: *const FField,
    pub name_private: FName,
    pub flags_private: EObjectFlags,
}

pub struct FProperty {
    // TODO
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


impl UObjectBase {
    pub fn get_path_name(&self, stop_outer: Option<&UObject>) -> String {
        let mut string = FString::new();
        unsafe {
            (globals().uobject_base_utility_get_path_name())(self, stop_outer, &mut string);
        }
        string.to_string()
    }
}
