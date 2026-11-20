//! Optional loose-file loading outside mounted paks.

use crate::Chivalry2::Engine::{
    FPakEntry, FPakFile, FPakListEntry, FPakPlatformFile, FString, TArray, TCHAR,
};
use widestring::U16CStr;
use winapi::um::fileapi::{GetFileAttributesW, INVALID_FILE_ATTRIBUTES};

#[unchained_macros::bind_func_detour(FPakPlatformFile::FindFileInPakFiles)]
pub(crate) unsafe fn find_file_in_pak_files_detour(
    this: *mut FPakPlatformFile,
    filename: *const TCHAR,
    out_pak_file: *mut *mut FPakFile,
    out_entry: *mut FPakEntry,
) -> u64 {
    let u16_wstr = unsafe { U16CStr::from_ptr_str(filename) };
    let u16_str = u16_wstr.to_string_lossy();
    let attr = unsafe { GetFileAttributesW(filename) };
    if attr != INVALID_FILE_ATTRIBUTES && u16_str.contains("../../../") {
        // swarn!("Blocked path traversal attempt: {}", u16_str);

        if !out_pak_file.is_null() {
            unsafe { *out_pak_file = std::ptr::null_mut() };
        }
        return 0;
    }
    unsafe { this.FindFileInPakFiles(filename, out_pak_file, out_entry) }
}

#[unchained_macros::bind_func_detour(FPakPlatformFile::FindFileInPakFiles_Static)]
pub(crate) unsafe fn find_file_in_pak_files_static_detour(
    paks: *mut TArray<FPakListEntry>,
    filename: *const TCHAR,
    out_pak_file: *mut *mut FPakFile,
    out_entry: *mut FPakEntry,
) -> u64 {
    let u16_wstr = unsafe { U16CStr::from_ptr_str(filename) };
    let u16_str = u16_wstr.to_string_lossy();
    let attr = unsafe { GetFileAttributesW(filename) };
    if attr != INVALID_FILE_ATTRIBUTES && u16_str.contains("../../../") {
        // swarn!("Blocked path traversal attempt: {}", u16_str);

        if !out_pak_file.is_null() {
            unsafe { *out_pak_file = std::ptr::null_mut() };
        }
        return 0;
    }
    unsafe {
        FPakPlatformFile::FindFileInPakFiles_Static(paks, filename, out_pak_file, out_entry)
    }
}

#[unchained_macros::bind_func_detour(FPakPlatformFile::IsNonPakFilenameAllowed)]
pub(crate) unsafe fn is_non_pak_filename_allowed_detour(
    _this: *mut FPakPlatformFile,
    _in_filename: *mut FString,
) -> u64 {
    1
}
