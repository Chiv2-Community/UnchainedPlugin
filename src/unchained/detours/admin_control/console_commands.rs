//! Administrative command execution logging.

use crate::chivalry2::engine::FString;
use crate::chivalry2::tbl::tbl_utility_library::TBLUtility;

#[unchained_macros::bind_func_detour(TBLUtility::execute_console_command)]
pub(crate) unsafe fn execute_console_command_detour(command: *const FString) {
    sinfo!("Executing: {}", unsafe { &*command });
    unsafe { TBLUtility::execute_console_command(command) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_binding_has_no_receiver_and_returns_void() {
        let _: unsafe fn(*const FString) = TBLUtility::execute_console_command;
        let _: unsafe fn(*const FString) = execute_console_command_detour;
    }
}
