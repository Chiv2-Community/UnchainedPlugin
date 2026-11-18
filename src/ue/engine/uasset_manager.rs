use crate::ue::IAssetRegistry;
use unchained_macros::bind_resolver_func;

/// Opaque native asset manager. No Rust allocation or layout is assumed.
#[repr(C)]
pub struct UAssetManager { _opaque: [u8; 0] }

impl UAssetManager {
    /// # Safety
    /// Self must be a live native asset manager; call on the engine thread.
    #[bind_resolver_func(crate::resolvers::asset_registry::GetAssetRegistry)]
    pub unsafe fn get_asset_registry(self: *const Self) -> *mut IAssetRegistry {}
}
