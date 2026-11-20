//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Serialization/Archive.h>.

/// Borrowed archive header; only native constructors may initialize it.
// Steam/EGS FLargeMemoryReader table 0x14499be88; UE Serialization/Archive.h.
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct FArchive {
    #[vtable(
        offset(0x20);
        /// # Safety
        /// Requires a live native archive, on its owning thread, without concurrent access.
        Tell(self: *mut Self) -> i64;
        /// # Safety
        /// Requires a live native archive, on its owning thread, without concurrent access.
        TotalSize(self: *mut Self) -> i64;
    )]
    vtable: *const (),
}
