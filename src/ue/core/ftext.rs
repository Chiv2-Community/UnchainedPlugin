use crate::ue::FString;
use unchained_macros::bind_resolver_func;

#[repr(C, align(8))]
pub struct FText {
    pub text_data: [u8; 0x10],
    pub flags: u32,
}

impl Default for FText {
    fn default() -> Self {
        Self { text_data: [0u8; 0x10], flags: 0 }
    }
}

impl FText {
    /// Native FText::ToString; the returned FString is borrowed from the text data.
    ///
    /// # Safety
    /// Self must be a live native FText on the engine thread. Do not free the
    /// result or mutate/invalidate the text data (including through native
    /// aliases or culture changes) while the returned borrow is live.
    #[bind_resolver_func(crate::resolvers::messages::broadcast_display::FText_ToString)]
    pub unsafe fn to_native_string<'a>(&'a self) -> &'a FString {}

    /// FText::AsCultureInvariant, with explicit hidden return storage.
    ///
    /// # Safety
    /// Output must be writable native FText storage. Call on the engine thread
    /// with an initialized allocator. The resolved moving-string overload may
    /// consume string: callers must not drop its backing buffer again.
    #[bind_resolver_func(crate::resolvers::admin_control::FText_AsCultureInvariant)]
    pub unsafe fn as_culture_invariant(output: *mut Self, string: *mut FString) -> *mut Self {}

    /// Release the native thread-safe shared reference (control block at +8).
    ///
    /// # Safety
    /// Self must own a live native shared reference, released exactly once.
    #[bind_resolver_func(crate::resolvers::messages::broadcast_display::FText_Release)]
    pub unsafe fn release(self: *mut Self) {}
}
