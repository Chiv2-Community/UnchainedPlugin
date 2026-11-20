//! Optional loose-file loading outside mounted paks.

use crate::chivalry2::engine::{
    FPakEntry, FPakFile, FPakListEntry, FPakPlatformFile, FString, TArray, TCHAR,
};
use widestring::U16CStr;
use winapi::um::fileapi::{GetFileAttributesW, INVALID_FILE_ATTRIBUTES};

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

#[unchained_macros::bind_func_detour(FPakPlatformFile::is_non_pak_filename_allowed)]
pub(crate) unsafe fn is_non_pak_filename_allowed_detour(
    _this: *mut FPakPlatformFile,
    _InFilename: *mut FString,
) -> u64 {
    1
}
