//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/PakFile/Public/IPlatformFilePak.h>.

#[cfg(feature = "loose_assets")]
use crate::chivalry2::engine::FString;
#[cfg(feature = "loose_assets")]
use crate::chivalry2::engine::TArray;
use crate::chivalry2::engine::TCHAR;
use std::sync::Mutex;
use widestring::U16CStr;

use std::ffi::c_void;
#[cfg(feature = "loose_assets")]
use winapi::um::fileapi::{GetFileAttributesW, INVALID_FILE_ATTRIBUTES};

// Signature source: owner-provided pattern for `FPakPlatformFile::Mount`
// on the new build (`FUN_142fc8d50`; confirmed = the call site's `bool Mount(const TCHAR*, uint32, const TCHAR*, bool)`).
// The RIP-relative displacement in the `48 8B 05` security-cookie load
// (immediately followed by `48 33 C4`) is wildcarded so the pattern survives
// rebuilds.
define_pattern_resolver!(PakPlatformFileMount, [
    "4C 8B DC 55 57 49 8D AB 78 FE FF FF 48 81 EC 78 02 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 30 01 00 00 0F B6 BD B0 01 00 00 33 C0"
]);

#[cfg(feature = "loose_assets")]
define_pattern_resolver!(
    FindFileInPakFiles_1,
    ["48 89 5C 24 ?? 48 89 6C 24 ?? 48 89 74 24 ?? 57 41 54 41 55 41 56 41 57 48 83 EC 30 33 FF"]
);

#[cfg(feature = "loose_assets")]
define_pattern_resolver!(
    FindFileInPakFiles_2,
    ["48 8B C4 4C 89 48 ?? 4C 89 40 ?? 48 89 48 ?? 55 53 48 8B EC"]
);

#[cfg(feature = "loose_assets")]
define_pattern_resolver!(
    IsNonPakFilenameAllowed,
    ["48 89 5C 24 ?? 48 89 6C 24 ?? 56 57 41 56 48 83 EC 30 48 8B F1 45 33 C0"]
);

/// Opaque engine-owned platform-file implementation; no Rust layout is assumed.
#[repr(C)]
pub struct FPakPlatformFile {
    _opaque: [u8; 0],
}

#[unchained_macros::runtime_bindings]
impl FPakPlatformFile {
    #[bind_resolved_func_detour(PakPlatformFileMount)]
    pub(crate) unsafe fn mount_detour(
        self: *mut Self,
        InPakFilename: *const TCHAR,
        PakOrder: u32,
        InPath: *const TCHAR,
        bLoadIndex: bool,
    ) -> bool {
        let ret = unsafe { self.mount(InPakFilename, PakOrder, InPath, bLoadIndex) };
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

    // FPakPlatformFile::FindFileInPakFiles(const TCHAR*, FPakFile**, FPakEntry*).
    #[cfg(feature = "loose_assets")]
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
    #[cfg(feature = "loose_assets")]
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

    #[cfg(feature = "loose_assets")]
    #[bind_resolved_func_detour(IsNonPakFilenameAllowed)]
    unsafe fn is_non_pak_filename_allowed_detour(
        self: *mut Self,
        _InFilename: *mut FString,
    ) -> u64 {
        1
    }
}

/// Paks the game has successfully mounted (recorded by the `Mount` detour on the
/// game thread during engine init, before our startup discovery runs).
pub static MOUNTED_PAKS: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub type FPakFile = c_void;

pub type FPakEntry = c_void;

// FPakPlatformFile::FPakListEntry.
pub type FPakListEntry = c_void;
