#![allow(dead_code)]

use std::{ptr, slice};
use windows::Win32::System::Memory::{
    VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
};

/// Change memory protection for a given region.
unsafe fn protect(addr: *mut u8, size: usize, new_protect: PAGE_PROTECTION_FLAGS) -> PAGE_PROTECTION_FLAGS {
    let mut old: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    let _ = VirtualProtect(addr as _, size, new_protect, &mut old);
    old
}

/// Overwrite memory with custom bytes.
pub unsafe fn patch(addr: *mut u8, data: &[u8]) {
    let size = data.len();
    let old = protect(addr, size, PAGE_EXECUTE_READWRITE);
    ptr::copy_nonoverlapping(data.as_ptr(), addr, size);
    protect(addr, size, old);
}

/// Fill memory with NOPs (0x90).
pub unsafe fn nop(addr: *mut u8, count: usize) {
    let old = protect(addr, count, PAGE_EXECUTE_READWRITE);
    ptr::write_bytes(addr, 0x90, count);
    protect(addr, count, old);
}

/// Read memory at address into a Vec<u8>.
pub unsafe fn read_memory(addr: *const u8, size: usize) -> Vec<u8> {
    slice::from_raw_parts(addr, size).to_vec()
}

/// Overwrite a pointer-sized value.
pub unsafe fn write_ptr<T>(addr: *mut T, value: T) {
    let size = std::mem::size_of::<T>();
    let old = protect(addr as *mut u8, size, PAGE_EXECUTE_READWRITE);
    ptr::write(addr, value);
    protect(addr as *mut u8, size, old);
}

/// Read a value from memory.
pub unsafe fn read_ptr<T: Copy>(addr: *const T) -> T {
    ptr::read(addr)
}
