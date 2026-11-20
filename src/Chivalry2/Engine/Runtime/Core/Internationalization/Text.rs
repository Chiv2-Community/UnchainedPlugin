//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Internationalization/Text.h>.

use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::TCHAR;

use std::{
    ffi::c_void,
    mem::{ManuallyDrop, MaybeUninit},
};


use binary_bloodhound::BinaryMatcher;

define_pattern_resolver!(
    FText__ToString,
    ["40 53 48 83 EC 20 48 8B D9 E8 ?? ?? ?? ?? 48 8B 0B 48 8B 01 48 83 C4 20 5B 48 FF 60 10"]
);

// FText::AsCultureInvariant(const FString&): select the copying overload.
// The old prologue also matched FromString; the flags-2 epilogue distinguishes
// culture-invariant text from FromString's flags-0x12 result. A new cache key
// prevents reuse of the old resolver's potentially moving-overload address.
define_custom_resolver!(FText__AsCultureInvariant, {
    BinaryMatcher::patterns(["41 83 4E 10 02 4C 8D 5C 24 60 49 8B 5B 20 49 8B C6"])?
        .root_functions()
        .addresses()
        .ensure_one()
});

// FText::~FText shares this entry with identical shared-reference destructors.
define_pattern_resolver!(FText__Destructor, [
    "40 53 48 83 EC 20 48 8B 59 08 48 85 DB 74 38 48 89 7C 24 30 BF FF FF FF FF 8B C7 F0 0F C1 43 08 83 F8 01 75 1D 48 8B 03 48 8B CB FF 10 F0 0F C1 7B 0C 83 FF 01 75 0B 48 8B 03 8B D7 48 8B CB FF 50 08 48 8B 7C 24 30 48 83 C4 20 5B C3"
]);

/// Opaque native text storage. Borrow native values through `from_ptr`;
/// construct owned culture-invariant values with `InvariantText::new`.
/// No zero-filled/default value represents a live UE FText.
#[unchained_macros::runtime_bindings]
#[repr(C, align(8))]
pub struct FText {
    text_data: *mut c_void,
    shared_reference: *mut c_void,
    flags: u32,
}

#[unchained_macros::runtime_bindings]
impl FText {
    /// Borrow native text without acquiring a shared reference.
    ///
    /// # Safety
    /// Non-null pointers must identify aligned, live UE-constructed FText
    /// storage for the returned lifetime. Its native shared reference must stay
    /// alive, and the inline storage must not be concurrently modified.
    pub unsafe fn from_ptr<'a>(text: *const Self) -> Option<&'a Self> {
        unsafe { text.as_ref() }
    }

    /// Returns an FString borrowed from the native text data.
    ///
    /// # Safety
    /// Self must be a live native FText. Access localized/history-driven text
    /// on the engine thread; unshared invariant text may use its owning thread.
    /// Do not free the result or mutate/invalidate its data (including through
    /// native aliases or culture changes) while the returned borrow is live.
    /// Synchronize native history access for the call, including invariant text.
    #[bind_resolved_func(self::FText__ToString)]
    pub unsafe fn ToString<'a>(&'a self) -> &'a FString {}

    /// Uses caller-provided return storage.
    /// Native overload: `(const FString&)`
    ///
    /// # Safety
    /// Output must be uninitialized writable native FText storage. String must
    /// be a valid native string, with the engine allocator/runtime initialized.
    /// The input is copied, not consumed. Output acquires one native reference.
    #[bind_resolved_func(self::FText__AsCultureInvariant)]
    unsafe fn AsCultureInvariant(output: *mut Self, string: &FString) -> *mut Self {}

    /// Destroy native text, releasing its thread-safe shared reference (control block at +8).
    ///
    /// # Safety
    /// Self must own a live native shared reference, released exactly once.
    #[bind_resolved_func(self::FText__Destructor)]
    pub unsafe fn Destructor(self: *mut Self) {}
}

/// One owned culture-invariant native text reference. Its display string is
/// independent of localization updates and cannot be mutated through safe Rust.
/// Native text and its owner are deliberately neither Send nor Sync.
pub struct InvariantText {
    text: FText,
}

impl InvariantText {
    /// Construct native text after engine globals and allocator initialization.
    pub fn new(text: &str) -> Self {
        let string = FString::from(text);
        let mut output = MaybeUninit::<FText>::uninit();
        // The verified const-reference overload copies the input and fully
        // initializes output; no zero-filled FText or ownership transfer occurs.
        unsafe {
            FText::AsCultureInvariant(output.as_mut_ptr(), &string);
            Self {
                text: output.assume_init(),
            }
        }
    }

    /// Borrow UTF-16 text without the trailing native terminator.
    pub fn as_utf16(&self) -> &[TCHAR] {
        // Only this type's constructor creates its text. Base text history never
        // rebuilds the display contents; persistence copies rather than moves
        // the old display buffer. Both remain alive until our reference drops.
        let string = unsafe { self.text.ToString() };
        let units = string.as_slice();
        units.strip_suffix(&[0]).unwrap_or(units)
    }

    pub fn to_string_lossy(&self) -> String {
        String::from_utf16_lossy(self.as_utf16())
    }

    /// Borrow a native argument. Native uses must preserve this owner's
    /// reference and immutable display contents, and synchronize history access
    /// with this owner. Dereferencing or passing this pointer to UE is unsafe.
    pub fn as_ptr(&self) -> *const FText {
        &self.text
    }

    /// Transfer the owned reference into native storage. FText has no Rust
    /// destructor; the recipient must release it or intentionally retain it.
    pub fn into_raw(self) -> FText {
        let owner = ManuallyDrop::new(self);
        unsafe { std::ptr::read(&owner.text) }
    }
}

impl Drop for InvariantText {
    fn drop(&mut self) {
        // Startup initializes the process-lifetime bindings. This owner holds
        // exactly one reference; borrowed native FText values never run this.
        unsafe {
            (&raw mut self.text).Destructor();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_native_text_import_is_absent() {
        assert!(unsafe { FText::from_ptr(std::ptr::null()) }.is_none());
    }

    #[test]
    fn native_text_has_no_default_clone_or_thread_transfer() {
        // Inference becomes ambiguous if any forbidden implementation is added.
        trait AmbiguousIfImplemented<A> {
            fn check() {}
        }
        impl<T: ?Sized> AmbiguousIfImplemented<()> for T {}
        struct IsSend;
        struct IsSync;
        struct IsClone;
        struct IsDefault;
        impl<T: ?Sized + Send> AmbiguousIfImplemented<IsSend> for T {}
        impl<T: ?Sized + Sync> AmbiguousIfImplemented<IsSync> for T {}
        impl<T: ?Sized + Clone> AmbiguousIfImplemented<IsClone> for T {}
        impl<T: ?Sized + Default> AmbiguousIfImplemented<IsDefault> for T {}
        let _ = <FText as AmbiguousIfImplemented<_>>::check;
        let _ = <InvariantText as AmbiguousIfImplemented<_>>::check;
    }
}
