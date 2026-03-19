use std::os::raw::c_void;
use crate::{backend_url, tools::hook_globals::cli_args, ue::{FString, FStringCopyError}};
use crate::events::models::{GameEvent, Join, Leave};
use crate::features::events::EVENT_SYSTEM;
use crate::game::chivalry2::{AController, ATBLGameMode};

define_pattern_resolver!(FString_AppendChars, [
    "45 85 C0 0F 84 89 00 00 00 48 89 5C 24 18 48 89 6C 24 20 56 48 83 EC 20 48 89 7C 24 30 48 8B EA 48 63 79 08 48 8B D9 4C 89 74 24 38 45 33 F6 85 FF 49 63 F0 41 8B C6 0F 94 C0 03 C7 03 C6 89 41 08 3B 41 0C 7E 07 8B D7 E8 ?? ?? ?? ?? 85 FF 49 8B C6 48 8B CF 48 8B D5 0F 95 C0 48 2B C8 48 8B 03 48 8D 1C 36 4C 8B C3 48 8D 3C 48 48 8B CF E8 ?? ?? ?? ?? 48 8B 6C 24 48 66 44 89 34 3B 4C 8B 74 24 38 48 8B 7C 24 30 48 8B 5C 24 40 48 83 C4 20 5E C3" // Universal
]);
CREATE_HOOK!(FString_AppendChars, CALLED, (), (this_ptr: *mut FString, str_ptr: *const u16, count: u32), {
});

define_pattern_resolver!(
    ATBLGameMode__PreLogin, [
        "4C 89 4C 24 ?? 48 89 54 24 ?? 48 89 4C 24 ?? 55 53 57 41 55"
    ]
);

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
    if value.is_empty() { None } else { Some(value) }
}

fn resolve_player_display_name(options: &str) -> String {
    extract_option_value(options, "Name")
        .or_else(|| extract_option_value(options, "PlayFabId"))
        .map(String::from)
        .unwrap_or_else(|| "Error: Player name and identifier unknown".to_string())
}

fn is_user_banned(addr: &str) -> bool {
    let mut suffix = "/api/v1/check-banned/".to_string();
    suffix.push_str(addr);
    let url = backend_url!(suffix);
    crate::sinfo!(f; "Checking Unchained ban status for {}", addr);
    let response = ureq::get(&url.to_string()).call();

    match response {
        Ok(res) => {
            match res.into_body().read_to_string() {
                Ok(body_str) => body_str.contains("true"),
                Err(error) => {
                    crate::swarn!(f; "Failed to read ban-check response for {}: {:?}", addr, error);
                    false
                }
            }
        }
        Err(e) => {
            crate::sinfo!(f; "Ban check failed: {:?}", e);
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
            LeaveNameResolutionError::NullPlayerState => "Logout controller had null PlayerState pointer",
            LeaveNameResolutionError::PlayerNameCopyFailed(error) => error.as_str(),
            LeaveNameResolutionError::EmptyPlayerName => "Logout PlayerState name was empty",
        }
    }
}

fn resolve_leave_name_from_controller(exiting_player: *mut AController) -> Result<String, LeaveNameResolutionError> {
    let controller = unsafe { exiting_player.as_ref() }
        .ok_or(LeaveNameResolutionError::NullController)?;

    let player_state = unsafe { controller.player_state.as_ref() }
        .ok_or(LeaveNameResolutionError::NullPlayerState)?;

    let display_name = player_state
        .player_name_private
        .copy_to_string()
        .map_err(LeaveNameResolutionError::PlayerNameCopyFailed)?;

    if display_name.trim().is_empty() {
        Err(LeaveNameResolutionError::EmptyPlayerName)
    } else {
        Ok(display_name)
    }
}
CREATE_HOOK!(ATBLGameMode__PreLogin, ACTIVE, NONE, (), (
    this_ptr: *mut crate::game::chivalry2::ATBLGameMode,
    options: *const FString,
    address: *const FString, 
    unique_id: *const c_void, // FUniqueNetIdRepl
    error_message: *mut FString
), {
    let options_string = unsafe { options.as_ref() }
        .and_then(|o| o.copy_to_string().ok())
        .unwrap_or_else(|| "<empty-string>".to_string());

    if !cli_args().use_backend_banlist || address.is_null() {
        let result = CALL_ORIGINAL!(ATBLGameMode__PreLogin(this_ptr, options, address, unique_id, error_message));
        if !error_message.is_null() && !(*error_message).is_empty() {
            return result;
        }

        let display_name = resolve_player_display_name(&options_string);
        crate::sinfo!("Player '{}' joined (backend banlist disabled)", display_name);
        EVENT_SYSTEM.game_event_publisher.publish(GameEvent::JoinEvent(Join { name: display_name }));
        return result;
    }

    let addr_string = unsafe { (*address).to_string() };
    crate::sinfo!("User joining with options '{}'", options_string);

    let original_result = CALL_ORIGINAL!(ATBLGameMode__PreLogin(this_ptr, options, address, unique_id, error_message));


    unsafe {
        if !error_message.is_null() && !(*error_message).is_empty() {
            return original_result;
        }
    }

    if is_user_banned(&addr_string) {
        let msg = "You are banned from this server.";
        let wide_msg: Vec<u16> = msg.encode_utf16().collect();
        
        unsafe {
            if error_message.is_null() {
                crate::swarn!(f; "Ban check returned banned for {}, but error_message was null", addr_string);
                return original_result;
            }

            o_FString_AppendChars.call(error_message, wide_msg.as_ptr(), wide_msg.len() as u32);
        }
        #[cfg(feature="verbose_hooks")]
        crate::swarn!(f; "User banned!");
    }
    else {
        let display_name = resolve_player_display_name(&options_string);
        crate::sinfo!("Player '{}' joined", display_name);
        EVENT_SYSTEM.game_event_publisher.publish(GameEvent::JoinEvent(Join { name: display_name }));
    }

    original_result
});

define_pattern_resolver!(ATBLGameMode__Logout, [
    "48 8B C4 55 56 41 54 41 57 48 8B EC"
]);
CREATE_HOOK!(ATBLGameMode__Logout, ACTIVE, NONE, (), (this_ptr: *mut ATBLGameMode, exiting_player: *mut AController), {
    crate::strace!(f; "ATBLGameMode::Logout hook triggered this_ptr={:?}, exiting_player={:?}", this_ptr, exiting_player);
    let leave_name = match resolve_leave_name_from_controller(exiting_player) {
        Ok(name) => {
            crate::sdebug!(f; "Resolved logout player name '{}'", name);
            name
        }
        Err(error) => {
            crate::swarn!(f; "{}. Publishing placeholder LeaveEvent.", error.as_str());
            "Unknown (logout name unavailable)".to_string()
        }
    };

    CALL_ORIGINAL!(ATBLGameMode__Logout(this_ptr, exiting_player));

    crate::sinfo!(f; "Player '{}' left", leave_name);
    EVENT_SYSTEM
        .game_event_publisher
        .publish(GameEvent::LeaveEvent(Leave { name: leave_name }));
});


// Technically we had no name for this in the disassembly. This just seems to be what it is doing.
define_pattern_resolver!(AllowAlternateBackend, [
   "48 8B C4 55 48 8D 68 ?? 48 81 EC C0 00 00 00 48 89 58 ?? 48 89 70 ?? 48 89 78 ?? 4C 89 60 ?? 4C 89 70"
]);
CREATE_PATCH!(AllowAlternateBackend, 0x4D, BYTES, &[0x90, 0xE9]);


// Unused
define_pattern_resolver!(ApproveLogin, [
    "48 89 5C 24 18 48 89 74 24 20 55 57 41 54 41 55 41 56 48 8D 6C 24 C9 48 81 EC A0 00 00 00 8B", // EGS
    "48 89 5C 24 10 48 89 74 24 18 55 57 41 54 41 56 41 57 48 8B EC 48 81 EC 80 00 00 00 8B", // STEAM
]);

define_pattern_resolver!(SendRequest, [
    "48 89 5C 24 ?? 48 89 74 24 ?? 48 89 7C 24 ?? 55 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 40 48 8B D9 49 8B F9"
]);
// TODO: check if some copying can be avoided
CREATE_HOOK!(SendRequest, ACTIVE, NONE, *mut c_void, (
    this_ptr: *mut c_void, u_ptr: *mut FString, body: *mut FString, a_key: *mut FString, a_val: *mut FString
), {
    // let original_url = unsafe { ManuallyDrop::new(std::ptr::read(u_ptr)) };
    let original_ptr = u_ptr;
    let url_w = unsafe { (*u_ptr).to_string() };
    // #[cfg(feature="verbose_hooks")]
    // crate::sinfo![f; "{}", url_w];

    let (target_url, mut target_auth) = if url_w == "https://EBF8D.playfabapi.com/Client/Matchmake?sdk=Chiv2_Version" {
        (Some(FString::from(backend_url!("/api/playfab/Client/Matchmake"))), Some(FString::from("")))
    } else if ["https://EBF8D.playfabapi.com/Match/GetMatchmakingTicket?sdk=Chiv2_Version"].iter().any(|&u| u == url_w) {
        (Some(FString::from("http://localhost")), None)
    } else {
        (None, None)
    };

    if let Some(new_url) = target_url {
            let original_bytes = unsafe { std::ptr::read(original_ptr) };
            unsafe { std::ptr::write(u_ptr, new_url); }
            let auth_ptr = match target_auth {
                Some(ref mut a) => a as *mut FString,
                None => a_val,
            };
 
            let res = CALL_ORIGINAL!(SendRequest(this_ptr, u_ptr, body, a_key, auth_ptr));
            unsafe { std::ptr::write(u_ptr, original_bytes); }

            return res;
    }

    CALL_ORIGINAL!(SendRequest(this_ptr, u_ptr, body, a_key, a_val))
});
