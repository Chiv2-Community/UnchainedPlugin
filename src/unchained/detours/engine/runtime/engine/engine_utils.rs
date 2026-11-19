//! Plugin replacements for `crate::chivalry2::engine::runtime::engine::engine_utils`.

use crate::chivalry2::engine::runtime::engine::engine_utils::*;
use crate::chivalry2::engine::FName;
use crate::chivalry2::engine::TCHAR;

#[unchained_macros::bind_func_detour(FConsoleOutputDevice::serialize)]
pub(crate) unsafe fn serialize_detour(
    this: *mut FConsoleOutputDevice,
    text: *const TCHAR,
    verbosity: u8,
    category: *const FName,
) {
    if !text.is_null() {
        let text = unsafe { widestring::U16CStr::from_ptr_str(text) };
        if !text
            .to_string_lossy()
            .starts_with("Command not recognized: RCON_INTERCEPT")
        {
            sinfo!("{}", text.display());
        }
    }
    unsafe {
        this.serialize(text, verbosity, category);
    }
}
