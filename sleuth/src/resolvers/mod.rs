#[macro_use]
mod macros;
// use dll_hook::ue;

use once_cell::sync::OnceCell;


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PlatformType {
    EGS,
    STEAM,
    XBOX,
    OTHER,
}

pub static PLATFORM: OnceCell<PlatformType> = OnceCell::new();
pub static BASE_ADDR: OnceCell<usize> = OnceCell::new();

pub fn current_platform() -> PlatformType {
    *PLATFORM.get().expect("Platform not initialized")
}

impl std::fmt::Display for PlatformType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::str::FromStr for PlatformType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "EGS" => Ok(PlatformType::EGS),
            "STEAM" => Ok(PlatformType::STEAM),
            "XBOX" => Ok(PlatformType::XBOX),
            "OTHER" => Ok(PlatformType::OTHER),
            _ => Err(()),
        }
    }
}
use std::ffi::OsStr;
use std::os::raw::c_int;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

#[repr(C)]
pub struct FString {
    pub str: *mut u16,       // wchar_t* == *mut u16 on Windows
    pub letter_count: c_int,
    pub max_letters: c_int,
}

impl FString {
    /// Create an FString from a Rust `&str` (or `String`)
    pub fn from_string(s: &str) -> (Self, Box<[u16]>) {
        // Convert to UTF-16 and null-terminate
        let wide: Vec<u16> = OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0)) // null terminator
            .collect();

        let letter_count = wide.len() as c_int;

        // Box it to keep it alive (heap allocation)
        let boxed = wide.into_boxed_slice();
        let ptr = boxed.as_ptr() as *mut u16;

        // Return both FString and the boxed data
        (
            FString {
                str: ptr,
                letter_count,
                max_letters: letter_count,
            },
            boxed,
        )
    }
    /// Creates an `FString` from a Rust wide string slice (`&[u16]` or `Vec<u16>`)
    pub fn new_from_wide_str(wide: &[u16]) -> Self {
        let letter_count = (wide.len()+1) as c_int;
        Self {
            str: wide.as_ptr() as *mut u16,
            letter_count,
            max_letters: letter_count,
        }
    }

    /// Creates an `FString` from a wide string pointer (like `*const u16`)
    /// WARNING: This assumes the pointer is null-terminated.
    pub unsafe fn from_ptr(ptr: *const u16) -> Self {
        if ptr.is_null() {
            return Self {
                str: null_mut(),
                letter_count: 0,
                max_letters: 0,
            };
        }

        // Count the number of wide characters (including null terminator)
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let letter_count = len as c_int + 1;

        Self {
            str: ptr as *mut u16,
            letter_count,
            max_letters: letter_count,
        }
    }
}


pub mod hook_retour;
pub mod admin_control;
pub mod asset_loading;
pub mod backend_hooks;
pub mod etc_hooks;
pub mod ownership_overrides;
pub mod unchained_integration;
pub mod rcon;

