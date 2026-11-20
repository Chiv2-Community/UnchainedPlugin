//! Track mounted paks and suppress signature-failure notifications.

use crate::chivalry2::engine::runtime::pak_file::i_platform_file_pak::*;
use crate::chivalry2::engine::TCHAR;
use std::sync::Mutex;
use widestring::U16CStr;

#[unchained_macros::bind_func_detour(FPakPlatformFile::broadcast_pak_chunk_signature_check_failure)]
pub(crate) unsafe fn broadcast_pak_chunk_signature_check_failure_detour(
    _data: *const FPakChunkSignatureCheckFailedData,
) {
}

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

// Filename-comparison branch patch in FPakPlatformFile::Mount.
CREATE_PATCH!(AuthenticateConsoleCommandCaller, BYTES, &[0xEB]);

/// Snapshot of the mounted pak filenames (absolute paths on Windows).
pub fn mounted_pak_filenames() -> Vec<String> {
    MOUNTED_PAKS.lock().unwrap().clone()
}

/// Paks the game has successfully mounted (recorded by the `Mount` detour on the
/// game thread during engine init, before our startup discovery runs).
pub static MOUNTED_PAKS: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_failure_replacement_preserves_static_native_signature() {
        let _: unsafe fn(*const FPakChunkSignatureCheckFailedData) =
            broadcast_pak_chunk_signature_check_failure_detour;
    }
}
