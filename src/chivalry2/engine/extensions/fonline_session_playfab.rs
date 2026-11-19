//! ArgonSDK main: [OnlineSubsystemPlayFab module](https://github.com/Chiv2-Community/ArgonSDK/tree/main/Plugins/OnlineSubsystemPlayFab/Source/OnlineSubsystemPlayFab).
//! The SDK omits the private FOnlineSessionPlayFab declaration.

use crate::chivalry2::engine::extensions::open_api_client_api::GetCurrentGamesPostResponse;

define_pattern_resolver!(
    OnGetCurrentGames_,
    ["48 89 4C 24 08 55 53 56 57 41 54 41 55 41 56 48 8D AC 24 60 FE FF FF"]
);

define_pattern_resolver!(PingSomeServers, [
    "4C 8B DC 49 89 4B 08 55 56 41 55 49 8D AB 38 FE FF FF 48 81 EC B0 02 00 00 4C 8D A9 40 03 00 00 48 8B F1"
]);

#[repr(C)]
pub struct FOnlineSessionPlayFab {
    _opaque: [u8; 0],
}

#[unchained_macros::runtime_bindings]
impl FOnlineSessionPlayFab {
    #[unchained_macros::bind_resolved_func(OnGetCurrentGames_)]
    pub(crate) unsafe fn on_get_current_games(
        self: *mut Self,
        response: *mut GetCurrentGamesPostResponse,
        extra: i32,
    ) {
    }
    #[unchained_macros::bind_resolved_func(PingSomeServers)]
    pub(crate) unsafe fn ping_some_servers(self: *mut Self, max_servers: i32) {}
}
