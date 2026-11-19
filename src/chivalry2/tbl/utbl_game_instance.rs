//! ArgonSDK main: [UTBLGameInstance](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLGameInstance.h).

use crate::chivalry2::engine::FLinearColor;
use crate::chivalry2::engine::FText;

define_pattern_resolver!(
    LoadFrontEndMap,
    ["48 8B C4 48 89 50 10 48 89 48 08 55 41 55 48 8D 68 98 48 81 EC 58 01 00 00 83 7A 08 00"]
);

define_pattern_resolver!(
    OnPreLoadMap,
    ["48 89 74 24 10 57 48 83 EC 50 83 B9 40 08 00 00 00 48 8D 35"]
);

define_pattern_resolver!(OnPostLoadMap,["40 55 53 56 57 41 56 41 57 48 8d ac 24 e8 fc ff ff 48 81 ec 18 04 00 00 48 8b 05 89 e7 09 04 48 33 c4 48 89 85 f0 02 00 00 33 c0 48 8b"]);

define_pattern_resolver!(
    TBLGameInstance_AddChatLine,
    ["48 89 6C 24 18 56 41 56 41 57 48 83 EC 40 49 63 E9 4D 8B F8 4C 8B F2 48 8B F1"]
);

mod startup_patch;

/// Opaque native game instance. Only live engine-owned pointers are valid.
#[repr(C)]
pub struct UTBLGameInstance {
    _opaque: [u8; 0],
}

#[unchained_macros::runtime_bindings]
impl UTBLGameInstance {
    #[unchained_macros::bind_resolved_func(LoadFrontEndMap)]
    pub(crate) unsafe fn load_front_end_map(
        self: *mut Self,
        _url: *mut crate::chivalry2::engine::FString,
    ) -> bool {
    }
    #[unchained_macros::bind_resolved_func(OnPreLoadMap)]
    pub(crate) unsafe fn on_pre_load_map(
        self: *mut Self,
        url: *mut crate::chivalry2::engine::FString,
    ) {
    }
    #[unchained_macros::bind_resolved_func(OnPostLoadMap)]
    pub(crate) unsafe fn on_post_load_map(
        self: *mut Self,
        world: *mut crate::chivalry2::engine::UWorld,
    ) {
    }

    /// UTBLGameInstance::AddChatLine, using borrowed native text/color inputs.
    ///
    /// # Safety
    /// Self must be live on the engine thread; text and color pointers must
    /// remain valid for the call. No ownership transfers.
    #[bind_resolved_func(self::TBLGameInstance_AddChatLine)]
    pub unsafe fn add_chat_line(
        self: *mut Self,
        sender: *const FText,
        message: *const FText,
        channel: u32,
        want_focus: bool,
        is_sender_dev: bool,
        origin_color: *const FLinearColor,
    ) {
    }
}
