//! Administrative command execution logging and permission bypass.

use crate::chivalry2::engine::FString;
use crate::chivalry2::tbl::tbl_utility_library::TBLUtility;

#[unchained_macros::bind_func_detour(TBLUtility::execute_console_command)]
pub(crate) unsafe fn execute_console_command_detour(command: *const FString) {
    sinfo!("Executing: {}", unsafe { &*command });
    unsafe { TBLUtility::execute_console_command(command) }
}

// Console-command permission bypass in UTBLLocalPlayer::Exec.
CREATE_PATCH!(UTBLLocalPlayer_Exec, BYTES, &[0xEB]);
CREATE_PATCH!(AuthenticateConsoleCommandCaller, BYTES, &[0xEB]);
