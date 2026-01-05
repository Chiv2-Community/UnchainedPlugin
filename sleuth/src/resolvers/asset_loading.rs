

// Allows loading loose assets instead of pak files
#[cfg(feature="loose_assets")]
mod loose_assets {
    
    use std::os::raw::c_void;
    use widestring::U16CStr;
    use winapi::um::fileapi::{GetFileAttributesW, INVALID_FILE_ATTRIBUTES};
    use windows::core::PCWSTR;

    define_pattern_resolver!(FindFileInPakFiles_1, [
        "48 89 5C 24 ?? 48 89 6C 24 ?? 48 89 74 24 ?? 57 41 54 41 55 41 56 41 57 48 83 EC 30 33 FF"
    ]);
    CREATE_HOOK!(FindFileInPakFiles_1, ACTIVE, NONE,
    u64, (
        this_ptr: *mut c_void, 
        Filename: *const u16, 
        OutPakFile: *mut *mut c_void, 
        OutEntry: *mut c_void), {
        let u16_wstr = unsafe { U16CStr::from_ptr_str(Filename) };
        let u16_str = u16_wstr.to_string_lossy();
        let attr = unsafe { GetFileAttributesW(Filename) };
        if attr != INVALID_FILE_ATTRIBUTES && u16_str.contains("../../../") {
            // crate::swarn!(f; "_1 Blocked path traversal attempt: {}", u16_str);
            
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
    CREATE_HOOK!(FindFileInPakFiles_2, ACTIVE, NONE,
    u64, (
        this_ptr: *mut c_void, 
        Filename: *const u16, 
        OutPakFile: *mut *mut c_void, 
        OutEntry: *mut c_void), {
        let u16_wstr = unsafe { U16CStr::from_ptr_str(Filename) };
        let u16_str = u16_wstr.to_string_lossy();
        let attr = unsafe { GetFileAttributesW(Filename) };
        if attr != INVALID_FILE_ATTRIBUTES && u16_str.contains("../../../") {
            // crate::swarn!(f; "_1 Blocked path traversal attempt: {}", u16_str);
            
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
    u64, (this_ptr: *mut c_void, InFilename: *mut c_void), {
        1
    });
}


