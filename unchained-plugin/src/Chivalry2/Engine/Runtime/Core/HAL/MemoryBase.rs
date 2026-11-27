//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/HAL/MemoryBase.h>.

use crate::Chivalry2::Engine::FGenericMemoryStats;
use crate::Chivalry2::Engine::FOutputDevice;
use crate::Chivalry2::Engine::UWorld;
use crate::Chivalry2::Engine::TCHAR;
use std::ffi::c_void;

/// Read UE's current allocator from its resolved pointer slot.
///
/// # Safety
/// Startup must bind the slot. Reads must not race with native writes, and the
/// current allocator must remain live throughout each native call.
/// Atomic binding publication does not synchronize native pointer replacement.
#[unchained_macros_core::bind_resolved_value(
    patternsleuth::resolvers::unreal::gmalloc::GMalloc,
    Value
)]
pub static GMalloc: *mut FMalloc;

// UE 4.25-plus 2d1aaeede678: HAL/MemoryBase.h and Misc/Exec.h.
// Confirmed against symbolized Binned2 and Steam/EGS vtable 0x144982A90.
#[unchained_macros_core::runtime_bindings]
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct FMalloc {
    #[vtable(
        /// MSVC deleting-destructor thunk, not a plain C++ destructor.
        ///
        /// # Safety
        /// Destroys the allocator even with flags == 0. Never call on borrowed GMalloc.
        /// Flags must match native ownership/allocation mode; the returned pointer is not live.
        Destructor(self: *mut Self, flags: u32) -> *mut c_void;
        /// # Safety
        /// Output must point to a live FOutputDevice; command must be NUL-terminated.
        Exec(self: *mut Self, world: *mut UWorld, command: *const TCHAR, output: *mut FOutputDevice) -> bool;
        Malloc(self: *mut Self, count: usize, alignment: u32) -> *mut c_void;
        TryMalloc(self: *mut Self, count: usize, alignment: u32) -> *mut c_void;
        Realloc(self: *mut Self, original: *mut c_void, count: usize, alignment: u32) -> *mut c_void;
        TryRealloc(self: *mut Self, original: *mut c_void, count: usize, alignment: u32) -> *mut c_void;
        Free(self: *mut Self, original: *mut c_void);
        QuantizeSize(self: *mut Self, count: usize, alignment: u32) -> usize;
        /// # Safety
        /// Original must belong to this allocator; size_out must be writable and non-null.
        /// SizeOut is meaningful only when this returns true.
        GetAllocationSize(self: *mut Self, original: *mut c_void, size_out: *mut usize) -> bool;
        /// # Safety
        /// Must run on the main thread, per FMalloc's native contract.
        Trim(self: *mut Self, trim_thread_caches: bool);
        SetupTLSCachesOnCurrentThread(self: *mut Self);
        ClearAndDisableTLSCachesOnCurrentThread(self: *mut Self);
        /// # Safety
        /// Requires native stats/name initialization; not safe during arbitrary preinit.
        InitializeStatsMetadata(self: *mut Self);
        UpdateStats(self: *mut Self);
        /// # Safety
        /// Stats must point to a live engine-constructed FGenericMemoryStats map.
        GetAllocatorStats(self: *mut Self, stats: *mut FGenericMemoryStats);
        /// # Safety
        /// Output must point to a live engine-constructed FOutputDevice.
        DumpAllocatorStats(self: *mut Self, output: *mut FOutputDevice);
        IsInternallyThreadSafe(self: *const Self) -> bool;
        ValidateHeap(self: *mut Self) -> bool;
        /// Returns borrowed NUL-terminated native text; do not free or mutate it.
        GetDescriptiveName(self: *mut Self) -> *const TCHAR;
    )]
    vtable: *const (),
}

unsafe impl Sync for FMalloc {}

unsafe impl Send for FMalloc {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Chivalry2::Engine::TArray;

    #[test]
    fn empty_arrays_need_neither_allocator_calls_nor_globals() {
        let mut native = FMalloc {
            vtable: std::ptr::null(),
        };
        let allocator = &raw mut native;
        let mut array = unsafe { TArray::<u16>::with_capacity_in(0, allocator) };
        unsafe { array.reserve_in(0, allocator) };
        assert!(array.as_ptr().is_null());
        assert!(unsafe { TArray::<u16>::from_slice_in(&[], allocator) }.is_empty());
        assert!(TArray::<u16>::with_capacity(0).is_empty());
        assert!(TArray::<u16>::from(&[][..]).is_empty());
    }

    #[test]
    fn oversized_array_capacity_is_rejected_before_allocation() {
        let mut native = FMalloc {
            vtable: std::ptr::null(),
        };
        let allocator = &raw mut native;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            TArray::<u16>::with_capacity_in(i32::MAX as usize + 1, allocator)
        }));
        assert!(result.is_err());
    }
}
