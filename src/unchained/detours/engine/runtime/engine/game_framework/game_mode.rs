//! Plugin replacements for `crate::chivalry2::engine::runtime::engine::game_framework::game_mode`.

use crate::chivalry2::engine::runtime::engine::game_framework::game_mode::*;
use crate::chivalry2::engine::FName;

#[unchained_macros::bind_func_detour(AGameMode::set_match_state)]
pub(crate) unsafe fn set_match_state_detour(this: *mut AGameMode, state: FName) {
    let name = state.to_string();
    sinfo!("Match state: {}", name);
    crate::features::motd::set_match_active(name == "InProgress");
    unsafe {
        this.set_match_state(state);
    }
}
