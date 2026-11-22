//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLSystemLibrary.h>.
//! Native owner: TBL/Source/TBL/TBLSystemLibrary.h.

use crate::Chivalry2::Engine::UObject;
use crate::Chivalry2::TBL::ATBLGameMode;

/// Static native helpers only; this is not a UObject instance layout.
#[unchained_macros_core::runtime_bindings]
pub struct UTBLSystemLibrary {
    _private: (),
}

#[unchained_macros_kismet::kismet_runtime_bindings("/Script/TBL.TBLSystemLibrary")]
impl UTBLSystemLibrary {
    /// # Safety
    /// Context must be a live native object on the engine thread. The returned
    /// game mode is borrowed and can be null in non-game worlds.
    pub unsafe fn GetTBLGameMode(context: *mut UObject) -> *mut ATBLGameMode {}
}
