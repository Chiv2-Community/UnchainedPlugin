//! Plugin replacements for `crate::chivalry2::engine::extensions::fonline_session_playfab`.

use crate::chivalry2::engine::extensions::fonline_session_playfab::*;
use crate::chivalry2::engine::extensions::open_api_client_api::GetCurrentGamesPostResponse;
use crate::tools::hook_globals::cli_args;
use std::cell::Cell;

#[unchained_macros::bind_func_detour(FOnlineSessionPlayFab::on_get_current_games, || !cli_args().is_server())]
pub(crate) unsafe fn on_get_current_games_detour(
    this: *mut FOnlineSessionPlayFab,
    response: *mut GetCurrentGamesPostResponse,
    extra: i32,
) {
    let already = IN_ON_GET_CURRENT_GAMES.with(|state| state.replace(true));
    unsafe {
        this.on_get_current_games(response, extra);
    }
    if !already {
        IN_ON_GET_CURRENT_GAMES.with(|state| state.set(false));
    }
}

#[unchained_macros::bind_func_detour(FOnlineSessionPlayFab::ping_some_servers, || !cli_args().is_server())]
pub(crate) unsafe fn ping_some_servers_detour(this: *mut FOnlineSessionPlayFab, max_servers: i32) {
    let already = IN_PING_SOME_SERVERS.with(|state| state.replace(true));
    strace!("PingSomeServers called");
    unsafe {
        this.ping_some_servers(max_servers);
    }
    if !already {
        IN_PING_SOME_SERVERS.with(|state| state.set(false));
    }
}

thread_local! {
    pub(crate) static IN_PING_SOME_SERVERS: Cell<bool> = const { Cell::new(false) };
    pub(crate) static IN_ON_GET_CURRENT_GAMES: Cell<bool> = const { Cell::new(false) };
}
