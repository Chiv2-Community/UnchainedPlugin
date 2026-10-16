use std::alloc::Layout;
use std::ffi::c_void;

#[derive(Debug)]
#[repr(C)]
pub struct FMalloc {
    vtable: *const FMallocVTable,
}
unsafe impl Sync for FMalloc {}
unsafe impl Send for FMalloc {}
impl FMalloc {
    /// Safety: The layout must be correct for the type being allocated.
    /// The allocator must be initialized and valid.
    pub unsafe fn malloc(&self, layout: Layout) -> *mut c_void {
        ((*self.vtable).malloc)(self, layout.size(), layout.align() as u32)
    }
    /// Safety: `original` must have been allocated by this allocator.
    /// The layout must be correct for the new allocation.
    pub unsafe fn realloc(&self, original: *mut c_void, layout: Layout) -> *mut c_void {
        ((*self.vtable).realloc)(self, original, layout.size(), layout.align() as u32)
    }
    /// Safety: `original` must have been allocated by this allocator and not yet freed.
    pub unsafe fn free(&self, original: *mut c_void) {
        if !original.is_null() {
            ((*self.vtable).free)(self, original)
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FMallocVTable {
    pub __vec_del_dtor: *const (),
    pub exec: *const (),
    pub malloc:
        unsafe extern "system" fn(this: &FMalloc, count: usize, alignment: u32) -> *mut c_void,
    pub try_malloc:
        unsafe extern "system" fn(this: &FMalloc, count: usize, alignment: u32) -> *mut c_void,
    pub realloc: unsafe extern "system" fn(
        this: &FMalloc,
        original: *mut c_void,
        count: usize,
        alignment: u32,
    ) -> *mut c_void,
    pub try_realloc: unsafe extern "system" fn(
        this: &FMalloc,
        original: *mut c_void,
        count: usize,
        alignment: u32,
    ) -> *mut c_void,
    pub free: unsafe extern "system" fn(this: &FMalloc, original: *mut c_void),
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
    use std::mem::ManuallyDrop;

    unsafe extern "C" {
        fn malloc(size: usize) -> *mut c_void;
        fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
        fn free(ptr: *mut c_void);
    }

    unsafe extern "system" fn allocate(_: &FMalloc, size: usize, _: u32) -> *mut c_void {
        malloc(size)
    }

    unsafe extern "system" fn reallocate(_: &FMalloc, ptr: *mut c_void, size: usize, _: u32) -> *mut c_void {
        realloc(ptr, size)
    }

    unsafe extern "system" fn deallocate(_: &FMalloc, ptr: *mut c_void) {
        free(ptr);
    }

    #[test]
    fn early_arrays_transfer_ownership_without_globals() {
        let vtable = FMallocVTable {
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
        };
        let allocator = FMalloc { vtable: &vtable };
        let mut folders = ManuallyDrop::new(TArray::<TArray<u16>>::new());
        unsafe {
            for units in [&[65u16, 0][..], &[66u16, 67, 0][..]] {
                folders.push_in(TArray::from_slice_in(units, &allocator), &allocator);
            }
            assert_eq!(folders.len(), 2);
            assert_eq!(folders[0].as_slice(), &[65, 0]);
            assert_eq!(folders[1].as_slice(), &[66, 67, 0]);

            // Simulate the engine freeing both levels, without invoking Rust Drop
            // (which requires plugin globals) on any engine-owned array.
            for folder in folders.as_slice() {
                allocator.free(folder.as_ptr() as *mut c_void);
            }
            allocator.free(folders.as_ptr() as *mut c_void);
        }
    }
}
