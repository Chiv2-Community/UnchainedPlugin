//! Host executable discovery for native binding and patch initialization.

use std::sync::LazyLock;

pub(crate) static BASE_ADDR: LazyLock<usize> = LazyLock::new(|| unsafe {
    windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
        .expect("Failed to get module handle")
        .0 as usize
});
