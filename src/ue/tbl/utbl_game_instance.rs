use crate::ue::{FLinearColor, FText};
use unchained_macros::bind_resolver_func;

define_pattern_resolver!(TBLGameInstance_AddChatLine, [
    "48 89 6C 24 18 56 41 56 41 57 48 83 EC 40 49 63 E9 4D 8B F8 4C 8B F2 48 8B F1"
]);

/// Opaque native game instance. Only live engine-owned pointers are valid.
#[repr(C)]
pub struct UTBLGameInstance { _opaque: [u8; 0] }

impl UTBLGameInstance {
    /// UTBLGameInstance::AddChatLine, using borrowed native text/color inputs.
    ///
    /// # Safety
    /// Self must be live on the engine thread; text and color pointers must
    /// remain valid for the call. No ownership transfers.
    #[bind_resolver_func(self::TBLGameInstance_AddChatLine)]
    pub unsafe fn add_chat_line(self: *mut Self, sender: *const FText, message: *const FText, channel: u32, want_focus: bool, is_sender_dev: bool, origin_color: *const FLinearColor) {}
}
