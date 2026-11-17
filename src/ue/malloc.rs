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
        /// Destroys the allocator even with flags == 0. Never call on borrowed GMalloc.
        /// Flags must match native ownership/allocation mode; the returned pointer is not live.
        deleting_destructor(this: *mut Self, flags: u32) -> *mut c_void;
        /// Output must point to a live FOutputDevice; command must be NUL-terminated.
        exec(this: *mut Self, world: *mut UWorld, command: *const TCHAR, output: *mut FOutputDevice) -> bool;
        malloc(this: *mut Self, count: usize, alignment: u32) -> *mut c_void;
        try_malloc(this: *mut Self, count: usize, alignment: u32) -> *mut c_void;
        realloc(this: *mut Self, original: *mut c_void, count: usize, alignment: u32) -> *mut c_void;
        try_realloc(this: *mut Self, original: *mut c_void, count: usize, alignment: u32) -> *mut c_void;
        free(this: *mut Self, original: *mut c_void);
        quantize_size(this: *mut Self, count: usize, alignment: u32) -> usize;
        /// Original must belong to this allocator; size_out must be writable and non-null.
        /// SizeOut is meaningful only when this returns true.
        get_allocation_size(this: *mut Self, original: *mut c_void, size_out: *mut usize) -> bool;
        /// Must run on the main thread, per FMalloc's native contract.
        trim(this: *mut Self, trim_thread_caches: bool);
        setup_tls_caches_on_current_thread(this: *mut Self);
        clear_and_disable_tls_caches_on_current_thread(this: *mut Self);
        /// Requires native stats/name initialization; not safe during arbitrary preinit.
        initialize_stats_metadata(this: *mut Self);
        update_stats(this: *mut Self);
        /// Stats must point to a live engine-constructed FGenericMemoryStats map.
        get_allocator_stats(this: *mut Self, stats: *mut FGenericMemoryStats);
        /// Output must point to a live engine-constructed FOutputDevice.
        dump_allocator_stats(this: *mut Self, output: *mut FOutputDevice);
        is_internally_thread_safe(this: *const Self) -> bool;
        validate_heap(this: *mut Self) -> bool;
        /// Returns borrowed NUL-terminated native text; do not free or mutate it.
        get_descriptive_name(this: *mut Self) -> *const TCHAR;
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
        let this = self.0.as_ptr();
        FMalloc::malloc(this, layout.size(), layout.align() as u32)
    }
    /// Safety: `original` must have been allocated by this allocator.
    /// The layout must be correct for the new allocation.
    #[inline(always)]
    pub unsafe fn realloc(self, original: *mut c_void, layout: Layout) -> *mut c_void {
        let this = self.0.as_ptr();
        FMalloc::realloc(this, original, layout.size(), layout.align() as u32)
    }
    /// Safety: `original` must have been allocated by this allocator and not yet freed.
    #[inline(always)]
    pub unsafe fn free(self, original: *mut c_void) {
        if !original.is_null() {
            let this = self.0.as_ptr();
            FMalloc::free(this, original)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::TArray;
    use std::cell::Cell;
    use std::mem::ManuallyDrop;

    #[repr(C)]
    struct TestAllocator {
        allocator: FMalloc,
        allocations: Cell<usize>,
        reallocations: Cell<usize>,
        frees: Cell<usize>,
    }

    impl TestAllocator {
        fn new(vtable: &FMallocVTable) -> Self {
            Self {
                allocator: FMalloc { vtable },
                allocations: Cell::new(0),
                reallocations: Cell::new(0),
                frees: Cell::new(0),
            }
        }

        fn handle(&self) -> FMallocHandle {
            FMallocHandle(NonNull::from(self).cast())
        }
    }

    unsafe extern "C" {
        fn malloc(size: usize) -> *mut c_void;
        fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
        fn free(ptr: *mut c_void);
    }

    unsafe extern "C" fn allocate(this: *mut FMalloc, size: usize, _: u32) -> *mut c_void {
        let state = &*this.cast::<TestAllocator>();
        state.allocations.set(state.allocations.get() + 1);
        malloc(size)
    }

    unsafe extern "C" fn reallocate(
        this: *mut FMalloc,
        ptr: *mut c_void,
        size: usize,
        _: u32,
    ) -> *mut c_void {
        let state = &*this.cast::<TestAllocator>();
        state.reallocations.set(state.reallocations.get() + 1);
        realloc(ptr, size)
    }

    unsafe extern "C" fn deallocate(this: *mut FMalloc, ptr: *mut c_void) {
        let state = &*this.cast::<TestAllocator>();
        state.frees.set(state.frees.get() + 1);
        free(ptr);
    }

    fn test_vtable() -> FMallocVTable {
        // Preserve allocation-behavior tests without exercising unrelated native slots.
        macro_rules! uncalled {
            ($name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?) => {
                unsafe extern "C" fn $name($($arg: $ty),*) $(-> $ret)? {
                    std::process::abort()
                }
            };
        }
        uncalled!(destroy(_this: *mut FMalloc, _flags: u32) -> *mut c_void);
        uncalled!(exec(_this: *mut FMalloc, _world: *mut UWorld, _cmd: *const TCHAR, _out: *mut FOutputDevice) -> bool);
        uncalled!(quantize(_this: *mut FMalloc, _count: usize, _alignment: u32) -> usize);
        uncalled!(allocation_size(_this: *mut FMalloc, _ptr: *mut c_void, _out: *mut usize) -> bool);
        uncalled!(trim(_this: *mut FMalloc, _caches: bool));
        uncalled!(update(_this: *mut FMalloc));
        uncalled!(stats(_this: *mut FMalloc, _stats: *mut FGenericMemoryStats));
        uncalled!(dump(_this: *mut FMalloc, _out: *mut FOutputDevice));
        uncalled!(thread_safe(_this: *const FMalloc) -> bool);
        uncalled!(validate(_this: *mut FMalloc) -> bool);
        uncalled!(name(_this: *mut FMalloc) -> *const TCHAR);
        FMallocVTable {
            deleting_destructor: destroy,
            exec,
            malloc: allocate,
            try_malloc: allocate,
            realloc: reallocate,
            try_realloc: reallocate,
            free: deallocate,
            quantize_size: quantize,
            get_allocation_size: allocation_size,
            trim,
            setup_tls_caches_on_current_thread: update,
            clear_and_disable_tls_caches_on_current_thread: update,
            initialize_stats_metadata: update,
            update_stats: update,
            get_allocator_stats: stats,
            dump_allocator_stats: dump,
            is_internally_thread_safe: thread_safe,
            validate_heap: validate,
            get_descriptive_name: name,
        }
    }

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
    fn shared_allocator_calls_update_native_implementation_state() {
        let vtable = test_vtable();
        let state = TestAllocator::new(&vtable);
        let value = state.handle().0.as_ptr();
        unsafe {
            let allocator = gmalloc_from_slot(&value).unwrap();
            let allocation = allocator.malloc(Layout::from_size_align(8, 8).unwrap());
            assert!(!allocation.is_null());
            assert_eq!(state.allocations.get(), 1);
            let allocation = allocator.realloc(allocation, Layout::from_size_align(16, 8).unwrap());
            assert!(!allocation.is_null());
            assert_eq!(state.reallocations.get(), 1);
            allocator.free(allocation);
            assert_eq!(state.frees.get(), 1);
            allocator.free(std::ptr::null_mut());
            assert_eq!(state.frees.get(), 1);
        }
    }

    #[test]
    fn shared_accessor_reads_late_initialization_without_globals() {
        let vtable = test_vtable();
        let state = TestAllocator::new(&vtable);
        let mut value: *mut FMalloc = std::ptr::null_mut();
        let slot = std::ptr::addr_of_mut!(value);
        unsafe {
            assert!(gmalloc_from_slot(std::ptr::null()).is_none());
            assert!(gmalloc_from_slot(slot).is_none());
            slot.write(state.handle().0.as_ptr());
            assert_eq!(gmalloc_from_slot(slot).unwrap().0, state.handle().0);
            slot.write(std::ptr::null_mut());
            assert!(gmalloc_from_slot(slot).is_none());
        }
    }

    #[test]
    fn early_arrays_transfer_ownership_without_globals() {
        let vtable = test_vtable();
        let state = TestAllocator::new(&vtable);
        let allocator = state.handle();
        let mut folders = ManuallyDrop::new(TArray::<TArray<u16>>::new());
        unsafe {
            for units in [&[65u16, 0][..], &[66u16, 67, 0][..]] {
                folders.push_in(TArray::from_slice_in(units, allocator), allocator);
            }
            assert_eq!(folders.len(), 2);
            assert_eq!(folders[0].as_slice(), &[65, 0]);
            assert_eq!(folders[1].as_slice(), &[66, 67, 0]);
            assert_eq!(state.allocations.get(), 2);
            assert_eq!(state.reallocations.get(), 2);
            assert_eq!(state.frees.get(), 0);

            // Simulate the engine freeing both levels, without invoking Rust Drop
            // (which requires plugin globals) on any engine-owned array.
            for folder in folders.as_slice() {
                allocator.free(folder.as_ptr() as *mut c_void);
            }
            allocator.free(folders.as_ptr() as *mut c_void);
            assert_eq!(state.frees.get(), 3);
        }
    }

    #[test]
    fn array_growth_policies_and_existing_capacity_are_preserved() {
        let vtable = test_vtable();
        let state = TestAllocator::new(&vtable);
        let allocator = state.handle();
        let mut exact = ManuallyDrop::new(TArray::<u16>::new());
        let mut rounded = ManuallyDrop::new(TArray::<u16>::new());
        unsafe {
            for value in 1..=3 {
                exact.push_in(value, allocator);
                rounded.reserve_in(1, allocator);
                // No globals are needed when push can reuse the reserved buffer.
                rounded.push(value);
            }
            assert_eq!(exact.capacity(), 3);
            assert_eq!(rounded.capacity(), 4);
            assert_eq!(exact.as_slice(), rounded.as_slice());
            let buffer = rounded.as_ptr();
            let reallocations = state.reallocations.get();
            rounded.reserve_in(0, allocator);
            rounded.reserve(1);
            rounded.push_in(4, allocator);
            assert_eq!(rounded.as_ptr(), buffer);
            assert_eq!(state.reallocations.get(), reallocations);
            assert_eq!(rounded.as_slice(), &[1, 2, 3, 4]);
            allocator.free(exact.as_ptr() as *mut c_void);
            allocator.free(rounded.as_ptr() as *mut c_void);
        }
    }

    #[test]
    fn empty_arrays_need_neither_allocator_calls_nor_globals() {
        let vtable = test_vtable();
        let state = TestAllocator::new(&vtable);
        let allocator = state.handle();
        let mut array = unsafe { TArray::<u16>::with_capacity_in(0, allocator) };
        unsafe { array.reserve_in(0, allocator) };
        assert!(array.as_ptr().is_null());
        assert!(unsafe { TArray::<u16>::from_slice_in(&[], allocator) }.is_empty());
        assert!(TArray::<u16>::with_capacity(0).is_empty());
        assert!(TArray::<u16>::from(&[][..]).is_empty());
        assert_eq!(state.allocations.get(), 0);
        assert_eq!(state.reallocations.get(), 0);
        assert_eq!(state.frees.get(), 0);
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
        let vtable = test_vtable();
        let state = TestAllocator::new(&vtable);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            TArray::<u16>::with_capacity_in(i32::MAX as usize + 1, state.handle())
        }));
        assert!(result.is_err());
        assert_eq!(state.allocations.get(), 0);
    }

    #[test]
    fn reserved_arrays_copy_and_clear_without_reallocating_or_freeing() {
        let vtable = test_vtable();
        let state = TestAllocator::new(&vtable);
        let allocator = state.handle();
        let source = [10u16, 20];
        let mut array = ManuallyDrop::new(unsafe { TArray::from_slice_in(&source, allocator) });
        assert_ne!(array.as_ptr(), source.as_ptr());
        assert_eq!(array.as_slice(), &source);
        assert_eq!(array.capacity(), source.len());
        array.clear();
        array.extend_from_slice(&[30, 40]);
        assert_eq!(array.as_slice(), &[30, 40]);
        assert_eq!(state.allocations.get(), 1);
        assert_eq!(state.reallocations.get(), 0);
        assert_eq!(state.frees.get(), 0);
        unsafe { allocator.free(array.as_ptr() as *mut c_void) };
        assert_eq!(state.frees.get(), 1);
    }
}
