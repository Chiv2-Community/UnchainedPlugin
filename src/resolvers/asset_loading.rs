use crate::ue::{FString, USaveGame};

// UGameplayStatics save/load, reflection-resolved (class binding lives in asset_registry.rs).
// Attach at FEngineLoop::Init; o_LoadGameFromSlot / o_SaveGameToSlot unchanged, so the
// TRY_CALL_ORIGINAL! trampoline callers in mod_management (settings load/save) are unaffected.

// USaveGame* UGameplayStatics::LoadGameFromSlot(const FString& SlotName, int32 UserIndex)
CREATE_HOOK_KISMET!(crate::resolvers::asset_registry::UGameplayStatics, "LoadGameFromSlot",
    LoadGameFromSlot, ACTIVE, *mut USaveGame, (arg0: *mut FString, arg1: i32), {
    let url_w = unsafe { (*arg0).to_string() };
    sdebug!["Triggered! {url_w}"];
});

// bool UGameplayStatics::SaveGameToSlot(USaveGame* SaveGameObject, const FString& SlotName, int32 UserIndex)
CREATE_HOOK_KISMET!(crate::resolvers::asset_registry::UGameplayStatics, "SaveGameToSlot",
    SaveGameToSlot, ACTIVE, bool, (savegame: *mut USaveGame, file: *mut FString, index: i32), {
    let url_w = unsafe { (*file).to_string() };
    sdebug!["Triggered! {url_w}"];
});


// Allows loading loose assets instead of pak files
#[cfg(feature = "loose_assets")]
mod loose_assets {

    use crate::ue::{FPakEntry, FPakFile, FPakListEntry, FPakPlatformFile, FString, TArray, TCHAR};
    use widestring::U16CStr;
    use winapi::um::fileapi::{GetFileAttributesW, INVALID_FILE_ATTRIBUTES};
    use windows::core::PCWSTR;

    define_pattern_resolver!(FindFileInPakFiles_1, [
        "48 89 5C 24 ?? 48 89 6C 24 ?? 48 89 74 24 ?? 57 41 54 41 55 41 56 41 57 48 83 EC 30 33 FF"
    ]);
    // FPakPlatformFile::FindFileInPakFiles(const TCHAR*, FPakFile**, FPakEntry*).
    CREATE_HOOK!(FindFileInPakFiles_1, ACTIVE, NONE,
    u64, (
        this_ptr: *mut FPakPlatformFile,
        Filename: *const TCHAR,
        OutPakFile: *mut *mut FPakFile,
        OutEntry: *mut FPakEntry), {
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
        CALL_ORIGINAL!(FindFileInPakFiles_1(this_ptr, Filename, OutPakFile, OutEntry))
    });

    define_pattern_resolver!(FindFileInPakFiles_2, [
        "48 8B C4 4C 89 48 ?? 4C 89 40 ?? 48 89 48 ?? 55 53 48 8B EC"
    ]);
    // Static FPakPlatformFile::FindFileInPakFiles(TArray<FPakListEntry>&, ...).
    CREATE_HOOK!(FindFileInPakFiles_2, ACTIVE, NONE,
    u64, (
        this_ptr: *mut TArray<FPakListEntry>,
        Filename: *const TCHAR,
        OutPakFile: *mut *mut FPakFile,
        OutEntry: *mut FPakEntry), {
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
        CALL_ORIGINAL!(FindFileInPakFiles_2(this_ptr, Filename, OutPakFile, OutEntry))
    });

    define_pattern_resolver!(IsNonPakFilenameAllowed, [
        "48 89 5C 24 ?? 48 89 6C 24 ?? 56 57 41 56 48 83 EC 30 48 8B F1 45 33 C0"
    ]);
    CREATE_HOOK!(IsNonPakFilenameAllowed, ACTIVE, NONE,
    u64, (this_ptr: *mut FPakPlatformFile, InFilename: *mut FString), {
        1
    });
}
