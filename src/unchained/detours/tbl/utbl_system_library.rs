//! Plugin replacements for `crate::chivalry2::tbl::utbl_system_library`.

#![allow(c_void_returns)]
use crate::chivalry2::tbl::utbl_system_library::*;

#[unchained_macros::bind_func_detour(UTBLSystemLibrary::get_game_info)]
pub(crate) unsafe fn get_game_info_detour(
    storage: *mut crate::chivalry2::engine::FString,
    world: *mut crate::chivalry2::engine::UWorld,
) -> *mut crate::chivalry2::engine::FString {
    let result = unsafe { UTBLSystemLibrary::get_game_info(storage, world) };
    sinfo!("{}", unsafe { (&*result).to_string() });
    result
}

#[unchained_macros::bind_func_detour(UTBLSystemLibrary::execute_console_command)]
pub(crate) unsafe fn execute_console_command_detour(
    command: *mut crate::chivalry2::engine::FString,
) -> std::ffi::c_void {
    sinfo!("Executing: {}", unsafe { &*command });
    unsafe { UTBLSystemLibrary::execute_console_command(command) }
}
