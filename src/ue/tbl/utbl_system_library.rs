use crate::ue::{ATBLGameMode, UObject};
use unchained_macros::bind_resolver_func;

/// Static native helpers only; this is not a UObject instance layout.
pub struct UTBLSystemLibrary { _private: () }

impl UTBLSystemLibrary {
    /// # Safety
    /// Context must be a live native object on the engine thread. The returned
    /// game mode is borrowed and can be null in non-game worlds.
    #[bind_resolver_func(crate::resolvers::etc_hooks::GetTBLGameMode)]
    pub unsafe fn get_tbl_game_mode(context: *mut UObject) -> *mut ATBLGameMode {}
}
