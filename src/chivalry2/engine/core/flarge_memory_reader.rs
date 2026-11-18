//! UE 4.25-plus: [FLargeMemoryReader](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Serialization/LargeMemoryReader.h).

use crate::chivalry2::engine::FName;
use unchained_macros::bind_resolved_func;

// Signature source: owner-provided target-build bytes for
// `FLargeMemoryReader::FLargeMemoryReader`. The constructor is used with
// `ELargeMemoryReaderFlags::None` so it borrows Rust-owned bytes during the
// synchronous registry load. Its rel32 call and RIP-relative vtable address
// are wildcarded.
define_pattern_resolver!(FLargeMemoryReaderCtor, [
    "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 48 83 EC 30 41 0F B6 F1 49 8B E8 48 8B DA 48 8B F9 E8 ?? ?? ?? ?? 80 A7 98 00 00 00 FE 48 8D 15 ?? ?? ?? ?? 40 0F B6 C6 48 89 17 24 01 48 89 9F A0 00 00 00"
]);

// Signature source: owner-provided target-build bytes for
// `FLargeMemoryReader::~FLargeMemoryReader`. Wildcard the RIP-relative vtable
// address, the `FMemory::Free` rel32 call, and the tail jump to base cleanup.
define_pattern_resolver!(FLargeMemoryReaderDtor, [
    "40 53 48 83 EC 20 F6 81 98 00 00 00 01 48 8D 05 ?? ?? ?? ?? 48 89 01 48 8B D9 74 0C 48 8B 89 A0 00 00 00 E8 ?? ?? ?? ?? 48 8B CB 48 83 C4 20 5B E9 ?? ?? ?? ??"
]);

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
    #[bind_resolved_func(self::FLargeMemoryReaderCtor)]
    pub unsafe fn construct(self: *mut Self, data: *const u8, size: i64, flags: u8, archive_name: FName) -> *mut Self {}

    /// Native non-deleting destructor; does not free the object's storage.
    ///
    /// # Safety
    /// Self must be a constructed reader, destroyed exactly once before its
    /// borrowed data expires, on the engine thread.
    #[bind_resolved_func(self::FLargeMemoryReaderDtor)]
    pub unsafe fn destruct(self: *mut Self) {}
}
