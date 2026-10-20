use std::alloc::Layout;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

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

#[derive(Debug)]
#[repr(C)]
pub struct FMalloc {
    vtable: *const FMallocVTable,
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
    pub unsafe fn malloc(self, layout: Layout) -> *mut c_void {
        let this = self.0.as_ptr();
        ((*(*this).vtable).malloc)(this, layout.size(), layout.align() as u32)
    }
    /// Safety: `original` must have been allocated by this allocator.
    /// The layout must be correct for the new allocation.
    pub unsafe fn realloc(self, original: *mut c_void, layout: Layout) -> *mut c_void {
        let this = self.0.as_ptr();
        ((*(*this).vtable).realloc)(this, original, layout.size(), layout.align() as u32)
    }
    /// Safety: `original` must have been allocated by this allocator and not yet freed.
    pub unsafe fn free(self, original: *mut c_void) {
        if !original.is_null() {
            let this = self.0.as_ptr();
            ((*(*this).vtable).free)(this, original)
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FMallocVTable {
    pub __vec_del_dtor: *const (),
    pub exec: *const (),
    // Native allocators mutate implementation state beyond the FMalloc prefix.
    // A Rust reference here would give the optimizer invalid aliasing guarantees.
    pub malloc:
        unsafe extern "system" fn(this: *mut FMalloc, count: usize, alignment: u32) -> *mut c_void,
    pub try_malloc:
        unsafe extern "system" fn(this: *mut FMalloc, count: usize, alignment: u32) -> *mut c_void,
    pub realloc: unsafe extern "system" fn(
        this: *mut FMalloc,
        original: *mut c_void,
        count: usize,
        alignment: u32,
    ) -> *mut c_void,
    pub try_realloc: unsafe extern "system" fn(
        this: *mut FMalloc,
        original: *mut c_void,
        count: usize,
        alignment: u32,
    ) -> *mut c_void,
    pub free: unsafe extern "system" fn(this: *mut FMalloc, original: *mut c_void),
    pub quantize_size: *const (),
    pub get_allocation_size: *const (),
    pub trim: *const (),
    pub setup_tls_caches_on_current_thread: *const (),
    pub clear_and_disable_tlscaches_on_current_thread: *const (),
    pub initialize_stats_metadata: *const (),
    pub update_stats: *const (),
    pub get_allocator_stats: *const (),
    pub dump_allocator_stats: *const (),
    pub is_internally_thread_safe: *const (),
    pub validate_heap: *const (),
    pub get_descriptive_name: *const (),
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

    unsafe extern "system" fn allocate(this: *mut FMalloc, size: usize, _: u32) -> *mut c_void {
        let state = &*this.cast::<TestAllocator>();
        state.allocations.set(state.allocations.get() + 1);
        malloc(size)
    }

    unsafe extern "system" fn reallocate(
        this: *mut FMalloc,
        ptr: *mut c_void,
        size: usize,
        _: u32,
    ) -> *mut c_void {
        let state = &*this.cast::<TestAllocator>();
        state.reallocations.set(state.reallocations.get() + 1);
        realloc(ptr, size)
    }

    unsafe extern "system" fn deallocate(this: *mut FMalloc, ptr: *mut c_void) {
        let state = &*this.cast::<TestAllocator>();
        state.frees.set(state.frees.get() + 1);
        free(ptr);
    }

    fn test_vtable() -> FMallocVTable {
        FMallocVTable {
            __vec_del_dtor: std::ptr::null(),
            exec: std::ptr::null(),
            malloc: allocate,
            try_malloc: allocate,
            realloc: reallocate,
            try_realloc: reallocate,
            free: deallocate,
            quantize_size: std::ptr::null(),
            get_allocation_size: std::ptr::null(),
            trim: std::ptr::null(),
            setup_tls_caches_on_current_thread: std::ptr::null(),
            clear_and_disable_tlscaches_on_current_thread: std::ptr::null(),
            initialize_stats_metadata: std::ptr::null(),
            update_stats: std::ptr::null(),
            get_allocator_stats: std::ptr::null(),
            dump_allocator_stats: std::ptr::null(),
            is_internally_thread_safe: std::ptr::null(),
            validate_heap: std::ptr::null(),
            get_descriptive_name: std::ptr::null(),
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
