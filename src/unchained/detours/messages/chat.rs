//! Capture player/system chat and dispatch chat commands.

use crate::chivalry2::engine::AActor;
use crate::chivalry2::engine::AController;
use crate::chivalry2::engine::FName;
use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::*;
use crate::chivalry2::tbl::{ATBLGameMode, ATBLPlayerController, ATBLPlayerState, EChatType};
use crate::events::models::GameChatMessage;
use crate::events::models::GameEvent;
use crate::events::models::NativeChat;
use crate::events::models::{
    ActorIdentity, ActorPermissions, ChatSource, CommandActor, CommandRequest, CommandSource,
    PermissionFlags,
};
use crate::features::events::EVENT_SYSTEM;
use crate::tools::command_parser::parse_command;
use regex::Regex;

#[unchained_macros::bind_func_detour(ATBLPlayerController::client_receive_chat)]
pub(crate) unsafe fn client_receive_chat_detour(
    this: *mut ATBLPlayerController,
    sender: *mut ATBLPlayerState,
    message: *const FString,
    chat_type: u32,
    is_sender_dev: bool,
    override_color: u32,
) {
    // Hosts capture once at BroadcastChat, not once per local recipient.
    if !has_authority() {
        if let Some(player) = unsafe { ATBLPlayerState::from_ptr(sender) } {
            capture_chat(player, message, chat_type);
        }
    }
    unsafe {
        this.client_receive_chat(sender, message, chat_type, is_sender_dev, override_color);
    }
}

#[unchained_macros::bind_func_detour(ATBLGameMode::broadcast_chat)]
pub(crate) unsafe fn broadcast_chat_detour(
    this: *mut ATBLGameMode,
    sender: *mut AActor,
    message: *const FString,
    chat_type: u32,
    override_color: u32,
) {
    if let Some(player) =
        unsafe { AActor::from_ptr(sender).and_then(|actor| actor_player_state(actor)) }
    {
        capture_chat(player, message, chat_type);
    }
    unsafe {
        this.broadcast_chat(sender, message, chat_type, override_color);
    }
}

pub(crate) fn has_authority() -> bool {
    crate::globals()
        .world()
        .is_some_and(|world| unsafe { !(*world).authority_game_mode.is_null() })
}

#[unchained_macros::bind_func_detour(APlayerController::client_message)]
pub(crate) unsafe fn client_message_detour(
    this: *mut APlayerController,
    S: *mut FString,
    Type: FName,
    MsgLifeTime: f32,
) {
    if let Some(message) = (unsafe { S.as_ref() }).and_then(|s| s.copy_to_string().ok()) {
        if let Some((chat_type, sender, message)) = system_message(&message) {
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
        this.client_message(S, Type, MsgLifeTime);
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
    let Some(message) = (unsafe { message.as_ref() }).and_then(|s| s.copy_to_string().ok()) else {
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
    let command = player_command_event(&sender, &message, chat_type);
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

fn player_command_event(sender: &str, message: &str, chat_type: EChatType) -> Option<GameEvent> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_replacement_uses_tbl_controller_receiver() {
        let _: unsafe fn(
            *mut ATBLPlayerController,
            *mut ATBLPlayerState,
            *const FString,
            u32,
            bool,
            u32,
        ) = client_receive_chat_detour;
    }

    #[test]
    fn global_player_command_preserves_arguments() {
        let Some(GameEvent::CommandRequestEvent(command)) =
            player_command_event("Player", "!vote kick\nreason", EChatType::AllSay)
        else {
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
            assert!(player_command_event("Player", "!vote kick", channel).is_none());
        }
    }

    #[test_case::test_case("hello\nworld"; "ordinary_chat")]
    #[test_case::test_case("!"; "empty_command")]
    fn global_noncommands_are_ignored(message: &str) {
        assert!(player_command_event("Player", message, EChatType::AllSay).is_none());
    }

    #[test]
    fn console_named_player_keeps_game_chat_permissions() {
        let Some(GameEvent::CommandRequestEvent(command)) =
            player_command_event("Console", "!cmd quit", EChatType::AllSay)
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
        assert_eq!(system_message(line), None);
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
        assert_eq!(system_message(line), Some((channel, sender, body)));
    }

    #[test_case::test_case(""; "empty")]
    #[test_case::test_case("Welcome!"; "plain_text")]
    #[test_case::test_case("Welcome\nPlayer <0>: text"; "player_header_inside_body")]
    #[test_case::test_case("<x>: text"; "nonnumeric_header")]
    #[test_case::test_case("<256>: Example <0>: text"; "overflow_header")]
    fn unrecognized_system_headers_remain_literal(line: &str) {
        assert_eq!(
            system_message(line),
            Some((EChatType::ServerSay, None, line))
        );
    }

    #[test]
    fn broadcast_envelope_is_system_metadata_not_a_player_command() {
        let body = "{$Server Console}\n!cmd quit";
        assert_eq!(
            system_message(&format!("<0>: {body}")),
            Some((EChatType::AllSay, Some("Server Console"), "!cmd quit"))
        );
        assert_eq!(system_message(&format!("Player <0>: {body}")), None);
        assert_eq!(
            system_message(&format!("<1>: {body}")),
            Some((EChatType::TeamSay, None, body))
        );
    }
}
