//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/UObjectGlobals.h>.

#![allow(non_snake_case)]
use crate::engine::*;
use widestring::U16CString;

pub unsafe fn get_uobject_from_path(path: &str, load_if_missing: bool) -> *mut UObject {
    let wide_path = U16CString::from_str(path).unwrap();

    if !load_if_missing {
        // Try to find it in memory first
        return crate::ue::static_find_object(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            wide_path.as_ptr(),
            false,
        );
    }

    crate::ue::static_load_object(
        std::ptr::null_mut(), // Any class
        std::ptr::null_mut(), // No specific outer
        wide_path.as_ptr(),
        std::ptr::null(),     // Filename (usually null)
        0,                    // LoadFlags (0 = LoadConfig)
        std::ptr::null_mut(), // Sandbox
        false,                // AllowNativeComponentClass
        std::ptr::null_mut(),
    )
}
