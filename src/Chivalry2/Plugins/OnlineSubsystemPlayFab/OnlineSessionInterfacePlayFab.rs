//! Native source: TBL/Plugins/OnlineSubsystemPlayFab/Source/OnlineSubsystemPlayFab/Public/OnlineSessionInterfacePlayFab.h.

use super::OpenAPIMerged::OpenAPIClientApi::GetCurrentGamesPostResponse;

define_pattern_resolver!(
    FOnlineSessionPlayFab__OnGetCurrentGames,
    ["48 89 4C 24 08 55 53 56 57 41 54 41 55 41 56 48 8D AC 24 60 FE FF FF"]
);

define_pattern_resolver!(FOnlineSessionPlayFab__PingSomeServers, [
    "4C 8B DC 49 89 4B 08 55 56 41 55 49 8D AB 38 FE FF FF 48 81 EC B0 02 00 00 4C 8D A9 40 03 00 00 48 8B F1"
]);

#[unchained_macros::runtime_bindings]
#[repr(C)]
pub struct FOnlineSessionPlayFab {
    _opaque: [u8; 0],
}

#[unchained_macros::runtime_bindings]
impl FOnlineSessionPlayFab {
    #[unchained_macros::bind_resolved_func(FOnlineSessionPlayFab__OnGetCurrentGames)]
    pub(crate) unsafe fn OnGetCurrentGames(
        self: *mut Self,
        response: *mut GetCurrentGamesPostResponse,
        extra: i32,
    ) {
    }
    #[unchained_macros::bind_resolved_func(FOnlineSessionPlayFab__PingSomeServers)]
    pub(crate) unsafe fn PingSomeServers(self: *mut Self, max_servers: i32) {}
}
