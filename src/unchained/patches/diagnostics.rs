//! Logging-only hooks; native behavior is unchanged.

use crate::chivalry2::engine::runtime::engine::engine_utils::FConsoleOutputDevice;
use crate::chivalry2::engine::runtime::engine::kismet::gameplay_statics::UGameplayStatics;
#[cfg(feature = "kismet_log")]
use crate::chivalry2::engine::{FFrame, UObject};
use crate::chivalry2::engine::{FName, FString, USaveGame, TCHAR};
use crate::chivalry2::tbl::tbl_utility_library::TBLUtility;
use crate::chivalry2::tbl::ui::user_feedback_and_bug_reports_library::UUserFeedbackAndBugReportsLibrary;

#[unchained_macros::bind_func_detour(UUserFeedbackAndBugReportsLibrary::get_game_info)]
pub(crate) unsafe fn get_game_info_detour(
    storage: *mut crate::chivalry2::engine::FString,
    world: *mut crate::chivalry2::engine::UWorld,
) -> *mut crate::chivalry2::engine::FString {
    let result = unsafe { UUserFeedbackAndBugReportsLibrary::get_game_info(storage, world) };
    sinfo!("{}", unsafe { (&*result).to_string() });
    result
}

#[unchained_macros::bind_func_detour(TBLUtility::execute_console_command)]
pub(crate) unsafe fn execute_console_command_detour(command: *const FString) {
    sinfo!("Executing: {}", unsafe { &*command });
    unsafe { TBLUtility::execute_console_command(command) }
}

#[unchained_macros::bind_func_detour(UGameplayStatics::load_game_from_slot)]
pub(crate) unsafe fn load_game_from_slot_detour<'a>(
    slot: &'a FString,
    index: i32,
) -> *mut USaveGame {
    sdebug!("Loading save game from slot {} (user index {})", slot, index);
    unsafe { UGameplayStatics::load_game_from_slot(slot, index) }
}

#[unchained_macros::bind_func_detour(UGameplayStatics::save_game_to_slot)]
pub(crate) unsafe fn save_game_to_slot_detour<'a>(
    game: *mut USaveGame,
    slot: &'a FString,
    index: i32,
) -> bool {
    sdebug!("Saving game to slot {} (user index {})", slot, index);
    unsafe { UGameplayStatics::save_game_to_slot(game, slot, index) }
}

#[unchained_macros::bind_func_detour(FConsoleOutputDevice::serialize)]
pub(crate) unsafe fn serialize_detour(
    this: *mut FConsoleOutputDevice,
    text: *const TCHAR,
    verbosity: u8,
    category: *const FName,
) {
    if !text.is_null() {
        let text = unsafe { widestring::U16CStr::from_ptr_str(text) };
        if !text
            .to_string_lossy()
            .starts_with("Command not recognized: RCON_INTERCEPT")
        {
            sinfo!("{}", text.display());
        }
    }
    unsafe {
        this.serialize(text, verbosity, category);
    }
}

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
