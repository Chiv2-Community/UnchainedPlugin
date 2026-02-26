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
