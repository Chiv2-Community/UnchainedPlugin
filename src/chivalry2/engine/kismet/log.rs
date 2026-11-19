//! UE 4.25-plus: [FFrame::KismetExecutionMessage](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Private/UObject/ScriptCore.cpp).

use super::FFrame;
use crate::chivalry2::engine::{FName, UObject, TCHAR};

define_pattern_resolver!(
    KismetExecutionMessage,
    ["48 89 5C 24 08 57 48 83 EC 30 0F B6 DA 48 8B F9 80 FA 01 ?? ?? ?? ?? ?? ?? ?? ?? ?? BA",]
);

static LIST_OF_SHAME: [&str; 4] = [
    "/Game/Maps/Frontend/CIT/FE_Citadel_Atmospherics.FE_Citadel_Atmospherics_C",
    "Divide by zero: ProjectVectorOnToVector with zero Target vector",
    "A null object was passed as a world context object to UEngine::GetWorldFromContextObject().",
    "/Game/Maps/Frontend/Blueprints/Customization_Rotation.Customization_Rotation_C",
];

#[unchained_macros::runtime_bindings]
impl FFrame {
    // Preserve the legacy return ABI pending native-signature verification.
    #[bind_resolved_func_detour(KismetExecutionMessage)]
    unsafe fn kismet_execution_message_detour(
        text: *const TCHAR,
        kind: u8,
        name: FName,
    ) -> *mut UObject {
        if !text.is_null() {
            let mut text = unsafe { widestring::U16CStr::from_ptr_str(text) }.to_string_lossy();
            if text.contains('\n') {
                text = format!("\n{text}");
            }
            if !LIST_OF_SHAME.iter().any(|entry| text.contains(entry)) {
                sdebug!("{text}");
            }
        }
        unsafe { Self::kismet_execution_message(text, kind, name) }
    }
}
