//! Plugin replacements for `crate::chivalry2::engine::pak_file::i_platform_file_pak`.

use crate::chivalry2::engine::pak_file::i_platform_file_pak::*;
#[cfg(feature = "loose_assets")]
use crate::chivalry2::engine::FString;
#[cfg(feature = "loose_assets")]
use crate::chivalry2::engine::TArray;
use crate::chivalry2::engine::TCHAR;
use std::sync::Mutex;
use widestring::U16CStr;
#[cfg(feature = "loose_assets")]
use winapi::um::fileapi::{GetFileAttributesW, INVALID_FILE_ATTRIBUTES};

#[unchained_macros::bind_func_detour(FPakPlatformFile::mount)]
pub(crate) unsafe fn mount_detour(
    this: *mut FPakPlatformFile,
    InPakFilename: *const TCHAR,
    PakOrder: u32,
    InPath: *const TCHAR,
    bLoadIndex: bool,
) -> bool {
    let ret = unsafe { this.mount(InPakFilename, PakOrder, InPath, bLoadIndex) };
    let path = if InPakFilename.is_null() {
        String::new()
    } else {
        unsafe { U16CStr::from_ptr_str(InPakFilename) }.to_string_lossy()
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

#[cfg(feature = "loose_assets")]
#[unchained_macros::bind_func_detour(FPakPlatformFile::find_file_in_pak_files)]
pub(crate) unsafe fn find_file_in_pak_files_detour(
    this: *mut FPakPlatformFile,
    Filename: *const TCHAR,
    OutPakFile: *mut *mut FPakFile,
    OutEntry: *mut FPakEntry,
) -> u64 {
    let u16_wstr = unsafe { U16CStr::from_ptr_str(Filename) };
    let u16_str = u16_wstr.to_string_lossy();
    let attr = unsafe { GetFileAttributesW(Filename) };
    if attr != INVALID_FILE_ATTRIBUTES && u16_str.contains("../../../") {
        // swarn!("Blocked path traversal attempt: {}", u16_str);

        if !OutPakFile.is_null() {
            unsafe { *OutPakFile = std::ptr::null_mut() };
        }
        return 0;
    }
    unsafe { this.find_file_in_pak_files(Filename, OutPakFile, OutEntry) }
}

#[cfg(feature = "loose_assets")]
#[unchained_macros::bind_func_detour(FPakPlatformFile::find_file_in_pak_files_static)]
pub(crate) unsafe fn find_file_in_pak_files_static_detour(
    this_ptr: *mut TArray<FPakListEntry>,
    Filename: *const TCHAR,
    OutPakFile: *mut *mut FPakFile,
    OutEntry: *mut FPakEntry,
) -> u64 {
    let u16_wstr = unsafe { U16CStr::from_ptr_str(Filename) };
    let u16_str = u16_wstr.to_string_lossy();
    let attr = unsafe { GetFileAttributesW(Filename) };
    if attr != INVALID_FILE_ATTRIBUTES && u16_str.contains("../../../") {
        // swarn!("Blocked path traversal attempt: {}", u16_str);

        if !OutPakFile.is_null() {
            unsafe { *OutPakFile = std::ptr::null_mut() };
        }
        return 0;
    }
    unsafe {
        FPakPlatformFile::find_file_in_pak_files_static(this_ptr, Filename, OutPakFile, OutEntry)
    }
}

#[cfg(feature = "loose_assets")]
#[unchained_macros::bind_func_detour(FPakPlatformFile::is_non_pak_filename_allowed)]
pub(crate) unsafe fn is_non_pak_filename_allowed_detour(
    _this: *mut FPakPlatformFile,
    _InFilename: *mut FString,
) -> u64 {
    1
}

/// Paks the game has successfully mounted (recorded by the `Mount` detour on the
/// game thread during engine init, before our startup discovery runs).
pub static MOUNTED_PAKS: Mutex<Vec<String>> = Mutex::new(Vec::new());
