//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Kismet/KismetSystemLibrary.h>.

#[cfg(any(feature = "kismet_server_identity_hooks", feature = "blueprint_printstring_log"))]
use crate::Chivalry2::Engine::UObject;
#[cfg(feature = "blueprint_printstring_log")]
use crate::Chivalry2::Engine::{FString, FText, FLinearColor};

/// Static helpers only; this is not a UObject instance layout.
#[unchained_macros_core::runtime_bindings]
pub struct UKismetSystemLibrary {
    _private: (),
}

#[cfg(feature = "kismet_server_identity_hooks")]
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Engine.KismetSystemLibrary")]
impl UKismetSystemLibrary {
    /// # Safety
    /// Run on the engine thread with a live world-context object.
    pub(crate) unsafe fn IsDedicatedServer(context: *mut UObject) -> bool {}

    /// # Safety
    /// Run on the engine thread with a live world-context object.
    pub(crate) unsafe fn IsServer(context: *mut UObject) -> bool {}

    /// # Safety
    /// Run on the engine thread with a live world-context object.
    pub(crate) unsafe fn IsStandalone(context: *mut UObject) -> bool {}
}

// Verified unique exec wrappers in the supported Steam build. These markers
// expose VM observations, not native-call ABIs or detourable folded bodies.
#[cfg(feature = "blueprint_printstring_log")]
#[unchained_macros_kismet::kismet_exec_bindings("/Script/Engine.KismetSystemLibrary")]
impl UKismetSystemLibrary {
    #[dummy]
    pub(crate) unsafe fn PrintWarning(message: &FString) {}

    #[dummy]
    pub(crate) unsafe fn PrintString(
        context: *mut UObject, message: &FString, print_to_screen: bool,
        print_to_log: bool, color: &FLinearColor, duration: f32,
    ) {}

    #[dummy]
    pub(crate) unsafe fn PrintText(
        context: *mut UObject, message: &FText, print_to_screen: bool,
        print_to_log: bool, color: &FLinearColor, duration: f32,
    ) {}
}
