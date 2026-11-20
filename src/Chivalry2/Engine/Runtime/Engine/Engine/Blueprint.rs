//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/Blueprint.h>.

use crate::Chivalry2::Engine::*;

use crate::Chivalry2::Engine::UClass;

use std::ffi::c_void;

#[unchained_macros::runtime_bindings]
#[rust_ctables::cpp_struct]
pub struct UBlueprint {
    #[inherit]
    pub base: UBlueprintCore,
    pub parent_class: *mut UClass,                  // 0x0050 (0x08)
    pub blueprint_type: u8,                         // 0x0058 (0x01)
    pub b_recompile_on_load: u8,                    // 0x0059 (0x01)
    pub b_has_been_regenerated: u8,                 // 0x005A (0x01)
    pub b_is_regenerating_on_load: u8,              // 0x005B (0x01)
    pub blueprint_system_version: i32,              // 0x005C (0x04)
    pub simple_construction_script: *mut c_void,    // 0x0060 (0x08)
    pub component_templates: TArray<*mut c_void>,   // 0x0068 (0x10)
    pub timelines: TArray<*mut c_void>,             // 0x0078 (0x10)
    pub component_class_overrides: TArray<u8>,      // 0x0088 (0x10)
    pub inheritable_component_handler: *mut c_void, // 0x0098 (0x08)
}
