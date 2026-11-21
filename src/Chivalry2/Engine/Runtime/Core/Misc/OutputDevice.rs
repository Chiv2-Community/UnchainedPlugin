//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Misc/OutputDevice.h>.

use crate::Chivalry2::Engine::*;

// Steam/EGS FConsoleOutputDevice table 0x1446d9b40; UE Misc/OutputDevice.h.
#[unchained_macros::runtime_bindings]
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct FOutputDevice {
    #[vtable(
        offset(0x8);
        /// # Safety
        /// Text must be NUL-terminated and category live. Obey the device's thread contract.
        /// Verbosity is the native uint8 level/flag mask; fatal levels may terminate the process.
        Serialize_WithTime(self: *mut Self, text: *const TCHAR, verbosity: u8, category: *const FName, time: f64);
        /// # Safety
        /// Text must be NUL-terminated and category live. Obey the device's thread contract.
        /// Verbosity is the native uint8 level/flag mask; fatal levels may terminate the process.
        Serialize(self: *mut Self, text: *const TCHAR, verbosity: u8, category: *const FName);
        /// # Safety
        /// Requires a live device and its permitted thread; no concurrent teardown.
        Flush(self: *mut Self);
    )]
    vtable: *const (),
    b_suppress_event_tag: bool,
    b_auto_emit_line_terminator: bool,
}
