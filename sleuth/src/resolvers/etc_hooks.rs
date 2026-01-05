use std::os::raw::c_void;
use crate::ue::FString;

define_pattern_resolver!(GetGameInfo, {
    // "48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00", // Universal
    // From sigga
    STEAM: ["48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC b0 01 00 00 45 33 FF"], // STEAM
    OTHER: ["48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00 33 FF"], // PDB
});

// FIXME: When is this called?
CREATE_HOOK!(GetGameInfo, ACTIVE, POST,
    *mut FString, (ret_ptr: *mut FString, uWorld: *mut c_void),
    |ret_val: *mut FString| {
        unsafe {
            crate::sinfo!(f; "{}", (&*ret_val).to_string());
        }
        ret_val
    });