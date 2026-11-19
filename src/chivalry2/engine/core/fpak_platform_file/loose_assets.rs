//! UE 4.25-plus: [FPakPlatformFile](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/PakFile/Public/IPlatformFilePak.h).

// Allows loading loose assets instead of pak files

use crate::chivalry2::engine::{FPakEntry, FPakFile, FPakListEntry, FPakPlatformFile, FString, TArray, TCHAR};
use widestring::U16CStr;
use winapi::um::fileapi::{GetFileAttributesW, INVALID_FILE_ATTRIBUTES};

define_pattern_resolver!(
    FindFileInPakFiles_1,
    ["48 89 5C 24 ?? 48 89 6C 24 ?? 48 89 74 24 ?? 57 41 54 41 55 41 56 41 57 48 83 EC 30 33 FF"]
);

define_pattern_resolver!(
    FindFileInPakFiles_2,
    ["48 8B C4 4C 89 48 ?? 4C 89 40 ?? 48 89 48 ?? 55 53 48 8B EC"]
);

define_pattern_resolver!(
    IsNonPakFilenameAllowed,
    ["48 89 5C 24 ?? 48 89 6C 24 ?? 56 57 41 56 48 83 EC 30 48 8B F1 45 33 C0"]
);

#[unchained_macros::runtime_bindings]
impl FPakPlatformFile {
    // FPakPlatformFile::FindFileInPakFiles(const TCHAR*, FPakFile**, FPakEntry*).
    #[bind_resolved_func_detour(FindFileInPakFiles_1)]
    unsafe fn find_file_in_pak_files_detour(
        self: *mut Self,
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
        unsafe { self.find_file_in_pak_files(Filename, OutPakFile, OutEntry) }
    }

    // Static FPakPlatformFile::FindFileInPakFiles(TArray<FPakListEntry>&, ...).
    #[bind_resolved_func_detour(FindFileInPakFiles_2)]
    unsafe fn find_file_in_pak_files_static_detour(
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
        unsafe { Self::find_file_in_pak_files_static(this_ptr, Filename, OutPakFile, OutEntry) }
    }

    #[bind_resolved_func_detour(IsNonPakFilenameAllowed)]
    unsafe fn is_non_pak_filename_allowed_detour(
        self: *mut Self,
        _InFilename: *mut FString,
    ) -> u64 {
        1
    }
}
