//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/BlueprintCore.h>.

use crate::Chivalry2::Engine::*;

use crate::Chivalry2::Engine::UClass;

#[unchained_macros_core::runtime_bindings]
#[rust_ctables::cpp_struct]
pub struct UBlueprintCore {
    #[inherit]
    pub base: UObject,
    pub skeleton_generated_class: *mut UClass, // 0x0028 (0x08)
    pub generated_class: *mut UClass,          // 0x0030 (0x08)

    // 0x50 - 0x38 = 0x18 (24 bytes)
    pub padding_metadata: [u8; 0x18], // 0x0038 (0x18)
}
