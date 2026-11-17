use crate::ue::FName;
use unchained_macros::bind_resolver_func;

/// Opaque native FLargeMemoryReader. Allocate only verified native storage;
/// this view is distinct from its FArchive base and has no Rust constructor.
#[repr(C)]
pub struct FLargeMemoryReader { _opaque: [u8; 0] }

impl FLargeMemoryReader {
    /// Native constructor; flags=0 borrows the data.
    ///
    /// # Safety
    /// Self must be writable, aligned storage of the verified native size. Data
    /// must remain valid until destruction. Use the engine thread/allocator.
    #[bind_resolver_func(crate::resolvers::asset_registry::FLargeMemoryReaderCtor)]
    pub unsafe fn construct(self: *mut Self, data: *const u8, size: i64, flags: u8, archive_name: FName) -> *mut Self {}

    /// Native non-deleting destructor; does not free the object's storage.
    ///
    /// # Safety
    /// Self must be a constructed reader, destroyed exactly once before its
    /// borrowed data expires, on the engine thread.
    #[bind_resolver_func(crate::resolvers::asset_registry::FLargeMemoryReaderDtor)]
    pub unsafe fn destruct(self: *mut Self) {}
}
