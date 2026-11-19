//! Plugin replacements for `crate::chivalry2::tbl::ui::user_feedback_and_bug_reports_library`.

use crate::chivalry2::tbl::ui::user_feedback_and_bug_reports_library::UUserFeedbackAndBugReportsLibrary;

#[unchained_macros::bind_func_detour(UUserFeedbackAndBugReportsLibrary::get_game_info)]
pub(crate) unsafe fn get_game_info_detour(
    storage: *mut crate::chivalry2::engine::FString,
    world: *mut crate::chivalry2::engine::UWorld,
) -> *mut crate::chivalry2::engine::FString {
    let result = unsafe { UUserFeedbackAndBugReportsLibrary::get_game_info(storage, world) };
    sinfo!("{}", unsafe { (&*result).to_string() });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chivalry2::engine::{FString, UWorld};

    #[test]
    fn static_binding_preserves_return_storage_abi() {
        type GetGameInfo = unsafe fn(*mut FString, *mut UWorld) -> *mut FString;
        let _: GetGameInfo = UUserFeedbackAndBugReportsLibrary::get_game_info;
        let _: GetGameInfo = get_game_info_detour;
    }
}
