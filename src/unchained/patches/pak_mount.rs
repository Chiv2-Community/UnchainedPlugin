//! Track mounted paks.

use crate::chivalry2::engine::runtime::pak_file::i_platform_file_pak::*;
use crate::chivalry2::engine::TCHAR;
use std::sync::Mutex;
use widestring::U16CStr;

#[unchained_macros::bind_func_detour(FPakPlatformFile::Mount)]
pub(crate) unsafe fn mount_detour(
    this: *mut FPakPlatformFile,
    in_pak_filename: *const TCHAR,
    pak_order: u32,
    in_path: *const TCHAR,
    load_index: bool,
) -> bool {
    let ret = unsafe { this.Mount(in_pak_filename, pak_order, in_path, load_index) };
    let path = if in_pak_filename.is_null() {
        String::new()
    } else {
        unsafe { U16CStr::from_ptr_str(in_pak_filename) }.to_string_lossy()
    };
    if ret {
        if !path.is_empty() {
            let mut paks = MOUNTED_PAKS.lock().unwrap();
            if !paks.contains(&path) {
                paks.push(path);
            }
        }
    } else if !path.is_empty() {
        sdebug!("[modmaps] mount failed (excluded): {}", path);
    }
    ret
}

/// Snapshot of the mounted pak filenames (absolute paths on Windows).
pub fn mounted_pak_filenames() -> Vec<String> {
    MOUNTED_PAKS.lock().unwrap().clone()
}

/// Paks the game has successfully mounted (recorded by the `Mount` detour on the
/// game thread during engine init, before our startup discovery runs).
pub static MOUNTED_PAKS: Mutex<Vec<String>> = Mutex::new(Vec::new());
