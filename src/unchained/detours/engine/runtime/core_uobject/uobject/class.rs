//! Plugin replacements for `crate::chivalry2::engine::runtime::core_uobject::uobject::class`.

use crate::chivalry2::engine::UClass;

#[unchained_macros::bind_func_detour(UClass::get_config_name, || crate::tools::hook_globals::cli_args().unchained_profile_location.is_some())]
pub(crate) unsafe fn get_config_name_detour(
    this: *mut UClass,
    storage: *mut crate::chivalry2::engine::FString,
) -> *mut crate::chivalry2::engine::FString {
    unsafe { this.get_config_name(storage) }
}
