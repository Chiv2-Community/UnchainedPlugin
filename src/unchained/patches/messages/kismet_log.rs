//! Optional Blueprint execution-message logging.

#[cfg(feature = "kismet_log")]
use crate::chivalry2::engine::FFrame;
#[cfg(feature = "kismet_log")]
use crate::chivalry2::engine::FName;
#[cfg(feature = "kismet_log")]
use crate::chivalry2::engine::UObject;
#[cfg(feature = "kismet_log")]
use crate::chivalry2::engine::TCHAR;

#[cfg(feature = "kismet_log")]
#[unchained_macros::bind_func_detour(FFrame::kismet_execution_message)]
pub(crate) unsafe fn kismet_execution_message_detour(
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
    unsafe { FFrame::kismet_execution_message(text, kind, name) }
}

#[cfg(feature = "kismet_log")]
static LIST_OF_SHAME: [&str; 4] = [
    "/Game/Maps/Frontend/CIT/FE_Citadel_Atmospherics.FE_Citadel_Atmospherics_C",
    "Divide by zero: ProjectVectorOnToVector with zero Target vector",
    "A null object was passed as a world context object to UEngine::GetWorldFromContextObject().",
    "/Game/Maps/Frontend/Blueprints/Customization_Rotation.Customization_Rotation_C",
];
