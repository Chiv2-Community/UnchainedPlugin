//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/PlayerController.h>.

#![allow(c_void_returns)]
use crate::chivalry2::engine::AActor;
use crate::chivalry2::engine::AController;
use crate::chivalry2::engine::FGuid;
use crate::chivalry2::engine::FName;
use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::UObjectType;
use crate::chivalry2::engine::*;
use crate::chivalry2::tbl::{ATBLPlayerState, EChatType};
use crate::events::models::GameChatMessage;
use crate::events::models::GameEvent;
use crate::events::models::NativeChat;
use crate::events::models::{
    ActorIdentity, ActorPermissions, ChatSource, CommandActor, CommandRequest, CommandSource,
    PermissionFlags,
};
use crate::features::events::EVENT_SYSTEM;
use crate::tools::command_parser::parse_command;
use crate::tools::hook_globals::globals;
use regex::Regex;
use std::ffi::c_void;

define_pattern_resolver!(DedicatedServerInit, [
    "41 56 48 83 EC 30 48 89 6C 24 48 48 89 74 24 50 48 89 7C 24 28 4C 89 7C 24 20 4C 8B F9 ?? ?? ?? ?? ?? 48 8B C8 4C 8B F0"
]);

define_pattern_resolver!(
    ClientReceiveChat,
    ["40 53 55 57 41 57 48 81 EC B8 00 00 00 41 8B E9 4D 8B F8 48 8B DA 48"]
);

define_pattern_resolver!(ClientMessage, [
    "4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 73 18 49 8B D8 49 89 43 C8 48 8B F1 49 89 43 D0 49 89 43 D8 49 8D 43"
]);

define_pattern_resolver!(
    ClientTravelInternal,
    ["4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 6B 10 41 0F B6 E9 49 89 73 18 48"]
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_player_command_preserves_arguments() {
        let Some(GameEvent::CommandRequestEvent(command)) = APlayerController::player_command_event(
            "Player",
            "!vote kick\nreason",
            EChatType::AllSay,
        ) else {
            panic!("missing command event");
        };
        assert_eq!(command.name, "vote");
        assert_eq!(command.args, ["kick", "reason"]);
        assert_eq!(command.raw_args, "kick\nreason");
    }

    #[test]
    fn nonglobal_channels_do_not_emit_player_commands() {
        for channel in EChatType::ALL
            .iter()
            .copied()
            .filter(|channel| *channel != EChatType::AllSay)
        {
            assert!(
                APlayerController::player_command_event("Player", "!vote kick", channel).is_none()
            );
        }
    }

    #[test_case::test_case("hello\nworld"; "ordinary_chat")]
    #[test_case::test_case("!"; "empty_command")]
    fn global_noncommands_are_ignored(message: &str) {
        assert!(
            APlayerController::player_command_event("Player", message, EChatType::AllSay).is_none()
        );
    }

    #[test]
    fn console_named_player_keeps_game_chat_permissions() {
        let Some(GameEvent::CommandRequestEvent(command)) =
            APlayerController::player_command_event("Console", "!cmd quit", EChatType::AllSay)
        else {
            panic!("missing command event");
        };
        assert_eq!(command.source, CommandSource::GameChat);
        assert!(matches!(
            command.actor.identity,
            ActorIdentity::GamePlayer { .. }
        ));
        assert_eq!(
            command.actor.permissions.flags,
            PermissionFlags::USER | PermissionFlags::START_VOTE
        );
    }

    #[test_case::test_case("Player Name <0>: [Console] Hello"; "console_prefix_in_body")]
    #[test_case::test_case("Player <1>:"; "empty_body")]
    #[test_case::test_case("Player <4294967296>: !vote kick\r\nreason"; "overflow_channel")]
    fn player_messages_are_ignored_without_parsing_their_body_or_channel(line: &str) {
        assert_eq!(APlayerController::system_message(line), None);
    }

    #[test_case::test_case("<0>: Hello", EChatType::AllSay, None, "Hello"; "global")]
    #[test_case::test_case("<1>:\t \r\nHello", EChatType::TeamSay, None, "\r\nHello"; "horizontal_space_only")]
    #[test_case::test_case("<6>:", EChatType::ServerSay, None, ""; "empty_body")]
    #[test_case::test_case("<6>:Welcome", EChatType::ServerSay, None, "Welcome"; "no_space")]
    #[test_case::test_case("<6>: Welcome\r\nExample <0>: !not_a_command\nEnjoy!", EChatType::ServerSay, None, "Welcome\r\nExample <0>: !not_a_command\nEnjoy!"; "multiline_motd")]
    #[test_case::test_case("<0>: [Console] Hello\r\nworld", EChatType::AllSay, Some("Console"), "Hello\r\nworld"; "console_sender")]
    fn system_headers_preserve_channel_sender_and_body(
        line: &str,
        channel: EChatType,
        sender: Option<&str>,
        body: &str,
    ) {
        assert_eq!(
            APlayerController::system_message(line),
            Some((channel, sender, body))
        );
    }

    #[test_case::test_case(""; "empty")]
    #[test_case::test_case("Welcome!"; "plain_text")]
    #[test_case::test_case("Welcome\nPlayer <0>: text"; "player_header_inside_body")]
    #[test_case::test_case("<x>: text"; "nonnumeric_header")]
    #[test_case::test_case("<256>: Example <0>: text"; "overflow_header")]
    fn unrecognized_system_headers_remain_literal(line: &str) {
        assert_eq!(
            APlayerController::system_message(line),
            Some((EChatType::ServerSay, None, line))
        );
    }

    #[test]
    fn broadcast_envelope_is_system_metadata_not_a_player_command() {
        let body = "{$Server Console}\n!cmd quit";
        assert_eq!(
            APlayerController::system_message(&format!("<0>: {body}")),
            Some((EChatType::AllSay, Some("Server Console"), "!cmd quit"))
        );
        assert_eq!(
            APlayerController::system_message(&format!("Player <0>: {body}")),
            None
        );
        assert_eq!(
            APlayerController::system_message(&format!("<1>: {body}")),
            Some((EChatType::TeamSay, None, body))
        );
    }
}

/// Partial native controller layout through Player; later fields are omitted.
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct APlayerController {
    #[inherit]
    pub base: crate::chivalry2::engine::AController,
    pub player: *mut UPlayer, // 0x02D0
}

#[unchained_macros::runtime_bindings]
impl APlayerController {
    #[bind_resolved_func_detour(DedicatedServerInit)]
    pub(crate) unsafe fn dedicated_server_init_detour(self: *mut Self) {}

    fn has_authority() -> bool {
        crate::globals()
            .world()
            .is_some_and(|world| unsafe { !(*world).authority_game_mode.is_null() })
    }

    #[bind_resolved_func_detour(ClientReceiveChat)]
    unsafe fn client_receive_chat_detour(
        self: *mut Self,
        sender: *mut ATBLPlayerState,
        message: *const FString,
        chat_type: u32,
        is_sender_dev: bool,
        override_color: u32,
    ) {
        // Hosts capture once at BroadcastChat, not once per local recipient.
        if !APlayerController::has_authority() {
            if let Some(player) = unsafe { ATBLPlayerState::from_ptr(sender) } {
                APlayerController::capture_chat(player, message, chat_type);
            }
        }
        unsafe {
            self.client_receive_chat(sender, message, chat_type, is_sender_dev, override_color);
        }
    }

    // void APlayerController::ClientMessage(const FString& S, FName Type, float MsgLifeTime)
    #[bind_resolved_func_detour(ClientMessage)]
    pub(crate) unsafe fn client_message_detour(
        self: *mut Self,
        S: *mut FString,
        Type: FName,
        MsgLifeTime: f32,
    ) {
        if let Some(message) = (unsafe { S.as_ref() }).and_then(|s| s.copy_to_string().ok()) {
            if let Some((chat_type, sender, message)) = APlayerController::system_message(&message)
            {
                EVENT_SYSTEM
                    .game_event_publisher
                    .publish(GameEvent::GameChatMessageEvent(GameChatMessage {
                        sender: sender.unwrap_or_default().into(),
                        message: message.into(),
                        chat_source: ChatSource::Native(NativeChat::System { channel: chat_type }),
                    }));
            }
        }
        unsafe {
            self.client_message(S, Type, MsgLifeTime);
        }
    }

    /// # Safety
    /// UObjectType::from_ptr's game-thread, lifetime and layout requirements apply.
    pub(crate) unsafe fn actor_player_state(sender: &AActor) -> Option<&ATBLPlayerState> {
        use crate::chivalry2::engine::APawn;
        // BroadcastChat accepts either a pawn or a controller, not a PlayerState.
        unsafe {
            let object = &sender.base;
            let player = if let Some(pawn) = object.uobj_cast::<APawn>() {
                pawn.player_state()?
            } else if let Some(controller) = object.uobj_cast::<AController>() {
                controller.player_state()?
            } else {
                return None;
            };
            player.base.base.base.uobj_cast::<ATBLPlayerState>()
        }
    }

    pub(crate) fn capture_chat(player: &ATBLPlayerState, message: *const FString, chat_type: u32) {
        let Some(message) = (unsafe { message.as_ref() }).and_then(|s| s.copy_to_string().ok())
        else {
            return;
        };
        let Some(chat_type) = u8::try_from(chat_type)
            .ok()
            .and_then(|value| EChatType::try_from(value).ok())
        else {
            swarn!("Invalid native chat type: {chat_type}");
            return;
        };
        let Ok(sender) = player.base.player_name_private.copy_to_string() else {
            return;
        };
        let command = APlayerController::player_command_event(&sender, &message, chat_type);
        EVENT_SYSTEM
            .game_event_publisher
            .publish(GameEvent::GameChatMessageEvent(GameChatMessage {
                sender,
                message,
                chat_source: ChatSource::Native(NativeChat::Player {
                    channel: chat_type,
                    faction: Some(player.faction),
                }),
            }));
        if let Some(event) = command {
            EVENT_SYSTEM.game_event_publisher.publish(event);
        }
    }

    fn player_command_event(
        sender: &str,
        message: &str,
        chat_type: EChatType,
    ) -> Option<GameEvent> {
        if chat_type != EChatType::AllSay {
            return None;
        }
        let content = message.strip_prefix('!')?;
        let (name, args, raw_args) = parse_command(content)?;
        let actor = CommandActor {
            display_name: sender.into(),
            identity: ActorIdentity::GamePlayer {
                player_id: 0, // TODO
                display_name: sender.into(),
            },
            permissions: ActorPermissions {
                flags: PermissionFlags::USER | PermissionFlags::START_VOTE, // TODO: Figure out when this is an admin user
            },
        };
        Some(GameEvent::CommandRequestEvent(CommandRequest {
            name,
            args,
            raw_args,
            actor,
            source: CommandSource::GameChat,
        }))
    }

    fn system_message(line: &str) -> Option<(EChatType, Option<&str>, &str)> {
        static RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
            Regex::new(r"^(?:(?P<player>[^\r\n]*?[ \t]+))??<(?P<channel>[0-9]+)>:[ \t]*").unwrap()
        });
        let (channel, body) = if let Some(caps) = RE.captures(line) {
            // Native hooks own player records; don't parse their channel or body.
            if caps.name("player").is_some() {
                return None;
            }
            match caps["channel"]
                .parse::<u8>()
                .ok()
                .and_then(|value| EChatType::try_from(value).ok())
            {
                Some(channel) => (channel, &line[caps.get(0).unwrap().end()..]),
                None => (EChatType::ServerSay, line),
            }
        } else {
            (EChatType::ServerSay, line)
        };
        // Broadcast envelopes carry sender metadata only on the system-chat path.
        if matches!(channel, EChatType::AllSay | EChatType::ServerSay) {
            if let Some((title, message, _)) =
                crate::events::broadcast_message::game_chat::broadcast_parts(body)
            {
                return Some((channel, Some(title), message));
            }
            if let Some(body) = body.strip_prefix("[Console] ") {
                return Some((channel, Some("Console"), body));
            }
        }
        Some((channel, None, body))
    }

    #[bind_resolved_func_detour(ClientTravelInternal)]
    unsafe fn client_travel_internal_detour(
        self: *mut Self,
        url: *mut FString,
        travel_type: u8,
        seamless: bool,
        guid: *mut FGuid,
    ) -> c_void {
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        struct ConnectionInfo {
            Address: String,
            PartyId: Option<String>,
            PartySize: Option<u32>,
            PlayFabId: Option<String>,
            UnofficialTicket: Option<String>,
        }
        fn parse_connection_info(raw: &str) -> Result<ConnectionInfo, Box<dyn std::error::Error>> {
            sinfo!("raw: {}", raw);
            let query = raw.replace('?', "&");
            Ok(serde_urlencoded::from_str(&format!("Address={query}"))?)
        }
        let string_ref = unsafe { &*url };
        let previous_world = globals().world();
        globals().set_world(std::ptr::null_mut());
        match previous_world {
            Some(world) => swarn!(
                "map travel started; invalidated globals.world ({:p}) and skipping world-dependent operations until post-load world is set", world
            ),
            None => sdebug!("map travel started with globals.world already None; world-dependent operations remain skipped until post-load world is set"),
        }
        match parse_connection_info(string_ref.to_string().as_str()) {
            Ok(info) => sinfo!("info: {:#?}", info),
            Err(error) => serror!("Error: {}", error),
        }
        unsafe { self.client_travel_internal(url, travel_type, seamless, guid) }
    }
}

unsafe impl UObjectType for APlayerController {
    const CLASS_NAME: &'static str = "PlayerController";
}
