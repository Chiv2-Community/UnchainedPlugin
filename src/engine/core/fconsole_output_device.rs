//! UE 4.25-plus: [console output-device contract](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Misc/OutputDeviceConsole.h).

use crate::ue::{FName, TCHAR};

#[repr(C)]
pub struct FConsoleOutputDevice {
    _opaque: [u8; 0],
}

define_pattern_resolver!(ConsoleOutputDevice__Serialize,["48 89 5C 24 10 48 89 6C 24 18 48 89 74 24 20 41 56 48 83 EC 30 41 0F B6 E8 49 8B D9 44 0F B6 C5 48 8B"]);

#[unchained_macros::runtime_bindings]
impl FConsoleOutputDevice {
    #[bind_resolved_func_detour(ConsoleOutputDevice__Serialize)]
    unsafe fn serialize_detour(
        self: *mut Self,
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
            self.serialize(text, verbosity, category);
        }
    }
}
