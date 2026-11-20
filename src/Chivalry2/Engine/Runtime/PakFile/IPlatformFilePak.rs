//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/PakFile/Public/IPlatformFilePak.h>.

#[cfg(feature = "loose_assets")]
use crate::Chivalry2::Engine::FString;
#[cfg(feature = "loose_assets")]
use crate::Chivalry2::Engine::TArray;
use crate::Chivalry2::Engine::TCHAR;

use std::ffi::c_void;

// Signature source: owner-provided pattern for `FPakPlatformFile::Mount`
// on the new build (`FUN_142fc8d50`; confirmed = the call site's `bool Mount(const TCHAR*, uint32, const TCHAR*, bool)`).
// The RIP-relative displacement in the `48 8B 05` security-cookie load
// (immediately followed by `48 33 C4`) is wildcarded so the pattern survives
// rebuilds.
define_pattern_resolver!(FPakPlatformFile__Mount, [
    "4C 8B DC 55 57 49 8D AB 78 FE FF FF 48 81 EC 78 02 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 30 01 00 00 0F B6 BD B0 01 00 00 33 C0"
]);

#[cfg(feature = "loose_assets")]
define_pattern_resolver!(
    FPakPlatformFile__FindFileInPakFiles,
    ["48 89 5C 24 ?? 48 89 6C 24 ?? 48 89 74 24 ?? 57 41 54 41 55 41 56 41 57 48 83 EC 30 33 FF"]
);

#[cfg(feature = "loose_assets")]
define_pattern_resolver!(
    FPakPlatformFile__FindFileInPakFiles_Static,
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
    #[unchained_macros::bind_resolved_func(FPakPlatformFile__Mount)]
    pub(crate) unsafe fn Mount(
        self: *mut Self,
        InPakFilename: *const TCHAR,
        PakOrder: u32,
        InPath: *const TCHAR,
        bLoadIndex: bool,
    ) -> bool {
    }

    #[cfg(feature = "loose_assets")]
    #[unchained_macros::bind_resolved_func(FPakPlatformFile__FindFileInPakFiles)]
    pub(crate) unsafe fn FindFileInPakFiles(
        self: *mut Self,
        Filename: *const TCHAR,
        OutPakFile: *mut *mut FPakFile,
        OutEntry: *mut FPakEntry,
    ) -> u64 {
    }

    // Static overload; receives the pak-list array rather than a platform-file object.
    #[cfg(feature = "loose_assets")]
    #[unchained_macros::bind_resolved_func(FPakPlatformFile__FindFileInPakFiles_Static)]
    pub(crate) unsafe fn FindFileInPakFiles_Static(
        Paks: *mut TArray<FPakListEntry>,
        Filename: *const TCHAR,
        OutPakFile: *mut *mut FPakFile,
        OutEntry: *mut FPakEntry,
    ) -> u64 {
    }

    #[cfg(feature = "loose_assets")]
    #[unchained_macros::bind_resolved_func(IsNonPakFilenameAllowed)]
    pub(crate) unsafe fn IsNonPakFilenameAllowed(
        self: *mut Self,
        _InFilename: *mut FString,
    ) -> u64 {
    }
}

pub type FPakFile = c_void;

pub type FPakEntry = c_void;

// FPakPlatformFile::FPakListEntry.
pub type FPakListEntry = c_void;
