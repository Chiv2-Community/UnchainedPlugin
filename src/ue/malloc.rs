use std::alloc::Layout;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use super::{FGenericMemoryStats, FOutputDevice, TCHAR};
use crate::game::engine::UWorld;

// Cache the slot, not the allocator: preinit can precede engine initialization.
static GMALLOC_SLOT: AtomicUsize = AtomicUsize::new(0);

/// # Safety
/// `slot` must be zero or the engine's process-lifetime GMalloc slot address.
/// Non-null slot values must point to process-lifetime allocators.
/// Slot contents must only change when no accessor reads them concurrently.
pub(crate) unsafe fn set_gmalloc_slot(slot: usize) {
    GMALLOC_SLOT.store(slot, Ordering::Release);
}

pub(crate) fn try_gmalloc() -> Option<FMallocHandle> {
    unsafe { gmalloc_from_slot(GMALLOC_SLOT.load(Ordering::Acquire) as *const *mut FMalloc) }
}

/// # Safety
/// A non-null slot must be readable and contain null or an allocator that remains
/// valid for every use of the returned handle.
/// The engine must not mutate the slot concurrently with this read.
unsafe fn gmalloc_from_slot(slot: *const *mut FMalloc) -> Option<FMallocHandle> {
    if slot.is_null() {
        None
    } else {
        NonNull::new(*slot).map(FMallocHandle)
    }
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
    fn shared_accessor_reads_late_initialization_without_globals() {
        let mut allocator = FMalloc { vtable: std::ptr::null() };
        let pointer = NonNull::from(&mut allocator);
        let mut value: *mut FMalloc = std::ptr::null_mut();
        let slot = std::ptr::addr_of_mut!(value);
        unsafe {
            assert!(gmalloc_from_slot(std::ptr::null()).is_none());
            assert!(gmalloc_from_slot(slot).is_none());
            slot.write(pointer.as_ptr());
            assert_eq!(gmalloc_from_slot(slot).unwrap().0, pointer);
            slot.write(std::ptr::null_mut());
            assert!(gmalloc_from_slot(slot).is_none());
        }
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
    #[should_panic(expected = "GMalloc is not initialized")]
    fn array_capacity_uses_early_allocator_accessor() {
        let _ = TArray::<u16>::with_capacity(1);
    }

    #[test]
    #[should_panic(expected = "GMalloc is not initialized")]
    fn string_conversion_uses_early_allocator_accessor() {
        let _ = crate::ue::FString::from("pak directory");
    }

    #[test]
    #[should_panic(expected = "GMalloc is not initialized")]
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
