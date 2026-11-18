use crate::ue::{ATBLGameMode, UObject};
use unchained_macros::bind_resolved_func;

define_pattern_resolver!(AuthenticateConsoleCommandCaller, [
    "74 ?? 40 32 FF E9 ?? ?? ?? ?? 49 8B 4D"
]);
CREATE_PATCH!(AuthenticateConsoleCommandCaller, BYTES, &[0xEB]);

define_pattern_resolver!(GetGameInfo, {
    STEAM: ["48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC b0 01 00 00 45 33 FF"],
    OTHER: ["48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00 33 FF"],
});

#[unchained_macros::bind_resolved_func_detour(GetGameInfo)]
unsafe fn get_game_info_detour(storage: *mut crate::ue::FString, world: *mut crate::ue::UWorld) -> *mut crate::ue::FString {
    let result = unsafe { get_game_info(storage, world) };
    sinfo!("{}", unsafe { (&*result).to_string() });
    result
}

// The native owner/receiver is not established; preserve the existing free-function ABI.
define_pattern_resolver!(ExecuteConsoleCommand, [
    "40 53 48 83 EC 30 48 8B 05 ?? ?? ?? ?? 48 8B D9 48 8B 90 58 0C 00 00"
]);

#[unchained_macros::bind_resolved_func_detour(ExecuteConsoleCommand)]
pub(crate) unsafe fn execute_console_command_detour(command: *mut crate::ue::FString) -> std::ffi::c_void {
    sinfo!("Executing: {}", unsafe { &*command });
    unsafe { execute_console_command(command) }
}

// ATBLGameMode * __cdecl UTBLSystemLibrary::GetTBLGameMode(UObject *param_1)
define_pattern_resolver![GetTBLGameMode, {
    EGS : ["40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51"], // EGS
    OTHER : [
        "40 53 48 83 EC 20 48 8B D9 48 85 C9 74 60 48 8B 01 FF 90 ?? ?? ?? ?? 48 85 C0 75 23 0F 1F 40 00 48 8B 5B 20 48 85 DB 74 11 48 8B 03 48 8B CB FF 90 ?? ?? ?? ?? 48 85 C0 74 E6 48 85 C0 74 2F 48 8B 98 28",
        "40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51" // EGS 2.11.4
        ]
}];

/// Static native helpers only; this is not a UObject instance layout.
pub struct UTBLSystemLibrary { _private: () }

impl UTBLSystemLibrary {
    /// # Safety
    /// Context must be a live native object on the engine thread. The returned
    /// game mode is borrowed and can be null in non-game worlds.
    #[bind_resolved_func(self::GetTBLGameMode)]
    pub unsafe fn get_tbl_game_mode(context: *mut UObject) -> *mut ATBLGameMode {}
}
