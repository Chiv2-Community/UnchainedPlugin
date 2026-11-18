//! ArgonSDK main: [ATBLGameMode](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLGameMode.h).

use super::{ATBLGameMode__Logout, ATBLGameMode__PreLogin};
use crate::events::models::{GameEvent, Join, Leave};
use crate::features::events::EVENT_SYSTEM;
#[cfg(feature = "server_registration")]
use crate::features::server_browser::http;
use crate::chivalry2::engine::AController;
use crate::chivalry2::tbl::ATBLGameMode;
use crate::chivalry2::engine::{FUniqueNetIdRepl, PlayerFlags};
use crate::tools::hook_globals::globals;
use crate::tools::hook_globals::cli_args;
use crate::chivalry2::engine::{FString, FStringCopyError};

/* seems to be broken, or just incorrect.
       Looks right in the binary, but keeps matching the wrong func
    XrefFirst,
    [patternsleuth::resolvers::unreal::util::utf8_pattern(
        " Minutes"
    )]
);
*/

/// Extracts a value from UE4 options strings of the form `?Key1=Val1?Key2=Val2`.
fn extract_option_value<'a>(options: &'a str, key: &str) -> Option<&'a str> {
    let search = format!("?{}=", key);
    let start = options.find(&search)? + search.len();
    let rest = &options[start..];
    let end = rest.find('?').unwrap_or(rest.len());
    let value = &rest[..end];
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn resolve_player_display_name(options: &str) -> String {
    extract_option_value(options, "Name")
        .or_else(|| extract_option_value(options, "PlayFabId"))
        .map(String::from)
        .unwrap_or_else(|| "Error: Player name and identifier unknown".to_string())
}

#[cfg(feature = "server_registration")]
fn is_user_banned(addr: &str) -> bool {
    sinfo!("Checking Unchained ban status for player");
    let response = std::panic::catch_unwind(|| {
        http::check_banned_sync(
            http::client(),
            addr,
            crate::features::tokio_runtime::TOKIO_RUNTIME.handle(),
        )
    })
    .ok()
    .flatten();
    match response {
        Some(banned) => banned,
        None => {
            swarn!("Ban check failed");
            false
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum LeaveNameResolutionError {
    NullController,
    NullPlayerState,
    PlayerNameCopyFailed(FStringCopyError),
    EmptyPlayerName,
}

impl LeaveNameResolutionError {
    fn as_str(self) -> &'static str {
        match self {
            LeaveNameResolutionError::NullController => "Logout received null AController pointer",
            LeaveNameResolutionError::NullPlayerState => {
                "Logout controller had null PlayerState pointer"
            }
            LeaveNameResolutionError::PlayerNameCopyFailed(error) => error.as_str(),
            LeaveNameResolutionError::EmptyPlayerName => "Logout PlayerState name was empty",
        }
    }
}

fn resolve_leave_name_from_controller(
    exiting_player: *mut AController,
) -> Result<Option<String>, LeaveNameResolutionError> {
    let controller =
        unsafe { exiting_player.as_ref() }.ok_or(LeaveNameResolutionError::NullController)?;

    let player_state = unsafe { controller.player_state.as_ref() }
        .ok_or(LeaveNameResolutionError::NullPlayerState)?;

    if player_state.player_flags.contains(PlayerFlags::IS_A_BOT) {
        return Ok(None);
    }

    let display_name = player_state
        .player_name_private
        .copy_to_string()
        .map_err(LeaveNameResolutionError::PlayerNameCopyFailed)?;

    if display_name.trim().is_empty() {
        Err(LeaveNameResolutionError::EmptyPlayerName)
    } else {
        Ok(Some(display_name))
    }
}
#[unchained_macros::runtime_bindings]
impl ATBLGameMode {
    #[bind_resolved_func_detour(ATBLGameMode__PreLogin)]
    pub(crate) unsafe fn pre_login_detour(
        self: *mut Self,
        options: *const FString,
        address: *const FString,
        unique_id: *const FUniqueNetIdRepl,
        error_message: *mut FString,
    ) {
        let this_ptr = self;
        let options_string = unsafe { options.as_ref() }
            .and_then(|o| o.copy_to_string().ok())
            .unwrap_or_else(|| "<empty-string>".to_string());

        if !cfg!(feature = "server_registration")
            || !cli_args().use_backend_banlist
            || address.is_null()
        {
            let result = unsafe { this_ptr.pre_login(options, address, unique_id, error_message) };
            if !error_message.is_null() && !(*error_message).is_empty() {
                return result;
            }

            let display_name = resolve_player_display_name(&options_string);
            sinfo!(
                "Player '{}' joined (backend banlist disabled)",
                display_name
            );
            EVENT_SYSTEM
                .game_event_publisher
                .publish(GameEvent::JoinEvent(Join { name: display_name }));
            return result;
        }

        #[cfg(feature = "server_registration")]
        let addr_string = unsafe { (*address).to_string() };
        sinfo!("User joining");

        let original_result =
            unsafe { this_ptr.pre_login(options, address, unique_id, error_message) };

        unsafe {
            if !error_message.is_null() && !(*error_message).is_empty() {
                return original_result;
            }
        }

        #[cfg(feature = "server_registration")]
        if is_user_banned(&addr_string) {
            let msg = "You are banned from this server.";
            let wide_msg: Vec<u16> = msg.encode_utf16().collect();

            unsafe {
                if error_message.is_null() {
                    swarn!(
                        "Ban check returned banned for {}, but error_message was null",
                        addr_string
                    );
                    return original_result;
                }

                error_message.append_chars(
                    wide_msg.as_ptr(),
                    i32::try_from(wide_msg.len()).expect("Login error message too long"),
                );
            }
            #[cfg(feature = "verbose_hooks")]
            swarn!("User banned!");
            return original_result;
        }
        let display_name = resolve_player_display_name(&options_string);
        sinfo!("Player '{}' joined", display_name);
        EVENT_SYSTEM
            .game_event_publisher
            .publish(GameEvent::JoinEvent(Join { name: display_name }));

        original_result
    }
}

#[unchained_macros::runtime_bindings]
impl ATBLGameMode {
    #[bind_resolved_func_detour(ATBLGameMode__Logout)]
    pub(crate) unsafe fn logout_detour(self: *mut Self, exiting_player: *mut AController) {
        let this_ptr = self;
        strace!(
            "Logout hook triggered this_ptr={:?}, exiting_player={:?}",
            this_ptr,
            exiting_player
        );

        if globals().world() == None {
            swarn!("globals.world is None; skipping leave event");
            return unsafe { this_ptr.logout(exiting_player) };
        }

        let leave_name = match resolve_leave_name_from_controller(exiting_player) {
            Ok(None) => {
                swarn!("Player is a bot; skipping leave event");
                return unsafe { this_ptr.logout(exiting_player) };
            }
            Ok(Some(name)) => {
                sdebug!("Resolved logout player name '{}'", name);
                name
            }
            Err(error) => {
                swarn!("{}. Publishing placeholder LeaveEvent.", error.as_str());
                "Unknown (logout name unavailable)".to_string()
            }
        };

        unsafe {
            this_ptr.logout(exiting_player);
        }

        sinfo!("Player '{}' left", leave_name);
        EVENT_SYSTEM
            .game_event_publisher
            .publish(GameEvent::LeaveEvent(Leave { name: leave_name }));
    }
}

// Disable stat increase to prevent stutters in offline play (and server mode?)
// bool __thiscall FTBLOnlineStats::IncStat(FTBLOnlineStats *this,int param_1,int param_2)
// define_pattern_resolver!(IncStat, [
//     "48 89 5C 24 10 48 89 6C 24 18 48 89 74 24 20 57 48 83 EC 20 80 79 10 00 41 8B E8 48 63 FA 48 8B D9 ?? ?? ?? ?? ?? ?? 48 8B 05 52 86 1F 04 80 3C" // Universal
// ]);
// CREATE_HOOK!(IncStat, ACTIVE, NONE, (),
//     (this_ptr: *mut FTBLOnlineStats, p1: i32, p2: i32),
// {
//     // sinfo!("Stat: {:?}", p1);
//     // CALL_ORIGINAL!(IncStat(this_ptr, p1, p2));
// });

// Disable stat change to prevent stutters in offline play (and server mode?)
// Nihi: Other OnlineStats-related functions could also impair performance. Needs some investigation. Alternatively, look into what exactly causes this (network connection?)
// bool __thiscall FTBLOnlineStats::SetStatInner(FTBLOnlineStats *this,int param_1,int param_2)
