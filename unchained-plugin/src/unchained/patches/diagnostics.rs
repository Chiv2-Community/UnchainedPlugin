//! Logging-only hooks; native behavior is unchanged.

use crate::Chivalry2::Engine::Runtime::Engine::EngineUtils::FConsoleOutputDevice;
use crate::Chivalry2::Engine::Runtime::Engine::Kismet::GameplayStatics::UGameplayStatics;
#[cfg(feature = "kismet_log")]
use crate::Chivalry2::Engine::{FFrame, UObject};
use crate::Chivalry2::Engine::{FName, FString, USaveGame, TCHAR};
use crate::Chivalry2::TBL::TBLUtilityLibrary::TBLUtility;
use crate::Chivalry2::TBL::UserFeedbackAndBugReportsLibrary::UUserFeedbackAndBugReportsLibrary;

#[unchained_macros_core::bind_func_detour(UUserFeedbackAndBugReportsLibrary::GetGameInfo)]
pub(crate) unsafe fn get_game_info_detour(
    storage: *mut crate::Chivalry2::Engine::FString,
    world: *mut crate::Chivalry2::Engine::UWorld,
) -> *mut crate::Chivalry2::Engine::FString {
    let result = unsafe { UUserFeedbackAndBugReportsLibrary::GetGameInfo(storage, world) };
    sinfo!("{}", unsafe { (&*result).to_string() });
    result
}

#[unchained_macros_core::bind_func_detour(TBLUtility::ExecuteConsoleCommand)]
pub(crate) unsafe fn execute_console_command_detour(command: *const FString) {
    sinfo!("Executing: {}", unsafe { &*command });
    unsafe { TBLUtility::ExecuteConsoleCommand(command) }
}

#[unchained_macros_core::bind_func_detour(UGameplayStatics::LoadGameFromSlot)]
pub(crate) unsafe fn load_game_from_slot_detour<'a>(
    slot: &'a FString,
    index: i32,
) -> *mut USaveGame {
    sdebug!("Loading save game from slot {} (user index {})", slot, index);
    unsafe { UGameplayStatics::LoadGameFromSlot(slot, index) }
}

#[unchained_macros_core::bind_func_detour(UGameplayStatics::SaveGameToSlot)]
pub(crate) unsafe fn save_game_to_slot_detour<'a>(
    game: *mut USaveGame,
    slot: &'a FString,
    index: i32,
) -> bool {
    sdebug!("Saving game to slot {} (user index {})", slot, index);
    unsafe { UGameplayStatics::SaveGameToSlot(game, slot, index) }
}

#[unchained_macros_core::bind_func_detour(FConsoleOutputDevice::Serialize)]
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
        this.Serialize(text, verbosity, category);
    }
}

#[cfg(feature = "kismet_log")]
#[unchained_macros_core::bind_func_detour(FFrame::KismetExecutionMessage)]
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
    unsafe { FFrame::KismetExecutionMessage(text, kind, name) }
}

#[cfg(feature = "kismet_log")]
static LIST_OF_SHAME: [&str; 4] = [
    "/Game/Maps/Frontend/CIT/FE_Citadel_Atmospherics.FE_Citadel_Atmospherics_C",
    "Divide by zero: ProjectVectorOnToVector with zero Target vector",
    "A null object was passed as a world context object to UEngine::GetWorldFromContextObject().",
    "/Game/Maps/Frontend/Blueprints/Customization_Rotation.Customization_Rotation_C",
];
