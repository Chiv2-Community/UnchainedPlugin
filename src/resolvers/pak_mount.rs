//! Pak-dir / mount discovery: records the actual `.pak` files the game mounts, so
//! mod discovery can run against the engine's real mounted-pak list instead of a
//! hardcoded `Content/Paks` scan.
//!
//! That solves two future problems:
//! - an extra directory the game loads paks from (captured automatically), and
//! - paks that exist on disk but are NOT loaded (e.g. missing `.sig`) — those
//!   never mount successfully, so they never appear here.
//!
//! `FPakPlatformFile::Mount` is the hookable, non-inline choke point every mount
//! goes through. (`GetMountedPakFilenames` exists but is `FORCEINLINE`, so it has
//! no standalone symbol to resolve.)

use crate::ue::{FPakPlatformFile, TCHAR};
use std::sync::Mutex;
use widestring::U16CStr;

/// Paks the game has successfully mounted (recorded by the `Mount` detour on the
/// game thread during engine init, before our startup discovery runs).
pub static MOUNTED_PAKS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Snapshot of the mounted pak filenames (absolute paths on Windows).
pub fn mounted_pak_filenames() -> Vec<String> {
    MOUNTED_PAKS.lock().unwrap().clone()
}

// Signature source: owner-provided pattern for `FPakPlatformFile::Mount`
// on the new build (`FUN_142fc8d50`; confirmed = the call site's `bool Mount(const TCHAR*, uint32, const TCHAR*, bool)`).
// The RIP-relative displacement in the `48 8B 05` security-cookie load
// (immediately followed by `48 33 C4`) is wildcarded so the pattern survives
// rebuilds.
define_pattern_resolver!(PakPlatformFileMount, [
    "4C 8B DC 55 57 49 8D AB 78 FE FF FF 48 81 EC 78 02 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 30 01 00 00 0F B6 BD B0 01 00 00 33 C0"
]);
CREATE_HOOK!(PakPlatformFileMount, ACTIVE, NONE, bool, (
    this_ptr: *mut FPakPlatformFile,
    InPakFilename: *const TCHAR,
    PakOrder: u32,
    InPath: *const TCHAR,
    bLoadIndex: bool
), {
    let ret = CALL_ORIGINAL!(PakPlatformFileMount(this_ptr, InPakFilename, PakOrder, InPath, bLoadIndex));
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
});
