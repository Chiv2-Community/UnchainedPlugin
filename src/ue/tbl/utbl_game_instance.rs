use crate::ue::{FLinearColor, FText};
use unchained_macros::bind_resolver_func;

/// Opaque native game instance. Only live engine-owned pointers are valid.
#[repr(C)]
pub struct UTBLGameInstance { _opaque: [u8; 0] }

impl UTBLGameInstance {
    /// UTBLGameInstance::AddChatLine, using borrowed native text/color inputs.
    ///
    /// # Safety
    /// Self must be live on the engine thread; text and color pointers must
    /// remain valid for the call. No ownership transfers.
    #[bind_resolver_func(crate::resolvers::messages::broadcast_display::TBLGameInstance_AddChatLine)]
    pub unsafe fn add_chat_line(self: *mut Self, sender: *const FText, message: *const FText, channel: u32, want_focus: bool, is_sender_dev: bool, origin_color: *const FLinearColor) {}
}
