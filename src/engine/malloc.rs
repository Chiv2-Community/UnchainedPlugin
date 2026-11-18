//! UE 4.25-plus: [FMalloc](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/HAL/MemoryBase.h).

use std::alloc::Layout;
use std::ffi::c_void;
use std::ptr::NonNull;
use super::{FGenericMemoryStats, FOutputDevice, TCHAR};
use crate::game::engine::UWorld;

// UE global FMalloc* GMalloc. Cache its slot, not the current allocator pointer.
/// # Safety
/// Startup must bind UE's process-lifetime allocator slot. Slot changes must not
/// race with reads; the allocator must remain live for all returned handles.
#[unchained_macros::bind_resolver_value(patternsleuth::resolvers::unreal::gmalloc::GMalloc, Value)]
unsafe fn native_gmalloc() -> *mut FMalloc {}

pub(crate) fn gmalloc() -> FMallocHandle {
    // The value binding panics on an unresolved slot or null allocator pointer.
    FMallocHandle(unsafe { NonNull::new_unchecked(native_gmalloc()) })
}

// UE 4.25-plus 2d1aaeede678: HAL/MemoryBase.h and Misc/Exec.h.
// Confirmed against symbolized Binned2 and Steam/EGS vtable 0x144982A90.
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct FMalloc {
    #[vtable(
        /// MSVC deleting-destructor thunk, not a plain C++ destructor.
        ///
        /// # Safety
        /// Destroys the allocator even with flags == 0. Never call on borrowed GMalloc.
        /// Flags must match native ownership/allocation mode; the returned pointer is not live.
        deleting_destructor(self: *mut Self, flags: u32) -> *mut c_void;
        /// # Safety
        /// Output must point to a live FOutputDevice; command must be NUL-terminated.
        exec(self: *mut Self, world: *mut UWorld, command: *const TCHAR, output: *mut FOutputDevice) -> bool;
        malloc(self: *mut Self, count: usize, alignment: u32) -> *mut c_void;
        try_malloc(self: *mut Self, count: usize, alignment: u32) -> *mut c_void;
        realloc(self: *mut Self, original: *mut c_void, count: usize, alignment: u32) -> *mut c_void;
        try_realloc(self: *mut Self, original: *mut c_void, count: usize, alignment: u32) -> *mut c_void;
        free(self: *mut Self, original: *mut c_void);
        quantize_size(self: *mut Self, count: usize, alignment: u32) -> usize;
        /// # Safety
        /// Original must belong to this allocator; size_out must be writable and non-null.
        /// SizeOut is meaningful only when this returns true.
        get_allocation_size(self: *mut Self, original: *mut c_void, size_out: *mut usize) -> bool;
        /// # Safety
        /// Must run on the main thread, per FMalloc's native contract.
        trim(self: *mut Self, trim_thread_caches: bool);
        setup_tls_caches_on_current_thread(self: *mut Self);
        clear_and_disable_tls_caches_on_current_thread(self: *mut Self);
        /// # Safety
        /// Requires native stats/name initialization; not safe during arbitrary preinit.
        initialize_stats_metadata(self: *mut Self);
        update_stats(self: *mut Self);
        /// # Safety
        /// Stats must point to a live engine-constructed FGenericMemoryStats map.
        get_allocator_stats(self: *mut Self, stats: *mut FGenericMemoryStats);
        /// # Safety
        /// Output must point to a live engine-constructed FOutputDevice.
        dump_allocator_stats(self: *mut Self, output: *mut FOutputDevice);
        is_internally_thread_safe(self: *const Self) -> bool;
        validate_heap(self: *mut Self) -> bool;
        /// Returns borrowed NUL-terminated native text; do not free or mutate it.
        get_descriptive_name(self: *mut Self) -> *const TCHAR;
    )]
    vtable: *const (),
}
unsafe impl Sync for FMalloc {}
unsafe impl Send for FMalloc {}
// Preserve the native object's provenance: a reference to its header would not
// permit allocator implementations to access mutable state beyond that header.
#[derive(Clone, Copy, Debug)]
pub struct FMallocHandle(NonNull<FMalloc>);

impl FMallocHandle {
    /// Safety: The layout must be correct for the type being allocated.
    /// The allocator must be initialized and valid.
    #[inline(always)]
    pub unsafe fn malloc(self, layout: Layout) -> *mut c_void {
        self.0.as_ptr().malloc(layout.size(), layout.align() as u32)
    }
    /// Safety: `original` must have been allocated by this allocator.
    /// The layout must be correct for the new allocation.
    #[inline(always)]
    pub unsafe fn realloc(self, original: *mut c_void, layout: Layout) -> *mut c_void {
        self.0.as_ptr().realloc(original, layout.size(), layout.align() as u32)
    }
    /// Safety: `original` must have been allocated by this allocator and not yet freed.
    #[inline(always)]
    pub unsafe fn free(self, original: *mut c_void) {
        if !original.is_null() {
            self.0.as_ptr().free(original)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::TArray;

    #[test]
    fn allocator_prefix_preserves_native_layout() {
        assert_eq!(
            std::mem::size_of::<FMalloc>(),
            std::mem::size_of::<*const FMallocVTable>()
        );
        assert_eq!(
            std::mem::align_of::<FMalloc>(),
            std::mem::align_of::<*const FMallocVTable>()
        );
    }

    #[test]
    #[should_panic(expected = "Required native value GMalloc is not bound")]
    fn allocator_access_before_binding_is_fatal() {
        let _ = gmalloc();
    }

    #[test]
    fn empty_arrays_need_neither_allocator_calls_nor_globals() {
        let mut native = FMalloc { vtable: std::ptr::null() };
        let allocator = FMallocHandle(NonNull::from(&mut native));
        let mut array = unsafe { TArray::<u16>::with_capacity_in(0, allocator) };
        unsafe { array.reserve_in(0, allocator) };
        assert!(array.as_ptr().is_null());
        assert!(unsafe { TArray::<u16>::from_slice_in(&[], allocator) }.is_empty());
        assert!(TArray::<u16>::with_capacity(0).is_empty());
        assert!(TArray::<u16>::from(&[][..]).is_empty());
    }

    #[test]
    #[should_panic(expected = "Required native value GMalloc is not bound")]
    fn array_capacity_uses_early_allocator_accessor() {
        let _ = TArray::<u16>::with_capacity(1);
    }

    #[test]
    #[should_panic(expected = "Required native value GMalloc is not bound")]
    fn string_conversion_uses_early_allocator_accessor() {
        let _ = crate::ue::FString::from("pak directory");
    }

    #[test]
    #[should_panic(expected = "Required native value GMalloc is not bound")]
    fn array_push_uses_early_allocator_accessor() {
        TArray::<u16>::new().push(1);
    }

    #[test]
    fn oversized_array_capacity_is_rejected_before_allocation() {
        let mut native = FMalloc { vtable: std::ptr::null() };
        let allocator = FMallocHandle(NonNull::from(&mut native));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            TArray::<u16>::with_capacity_in(i32::MAX as usize + 1, allocator)
        }));
        assert!(result.is_err());
    }
}
