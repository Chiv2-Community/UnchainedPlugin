//! Native owner: TBL/Source/TBL/TBLUtilityLibrary.h/.cpp.
//! Native source unavailable: the SDK's TBLUtilityLibrary.h declares UTBLUtilityLibrary,
//! not the TBLUtility namespace or ExecuteConsoleCommand.

use crate::Chivalry2::Engine::FString;

// Shipping RVA 0x18b8cc0.
define_pattern_resolver!(
    TBLUtility__ExecuteConsoleCommand,
    ["40 53 48 83 EC 30 48 8B 05 ?? ?? ?? ?? 48 8B D9 48 8B 90 58 0C 00 00"]
);

/// Rust adapter for the native TBLUtility namespace, not a UObject class.
#[unchained_macros::runtime_bindings]
pub struct TBLUtility {
    _private: (),
}

#[unchained_macros::runtime_bindings]
impl TBLUtility {
    #[unchained_macros::bind_resolved_func(TBLUtility__ExecuteConsoleCommand)]
    pub(crate) unsafe fn ExecuteConsoleCommand(command: *const FString) {}
}
