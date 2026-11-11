use crate::{
    game::{
        chivalry2::{ATBLGameMode, EChatType},
        engine::FText,
    },
    ue::{FConsoleOutputDevice, TCHAR},
};

mod broadcast_display;

mod client_message {
    use crate::events::models::{
        ActorIdentity, ActorPermissions, ChatSource, CommandActor, CommandRequest,
        CommandSource, PermissionFlags,
    };
    use crate::features::events::EVENT_SYSTEM;
    use crate::tools::command_parser::parse_command;
    use crate::ue::APlayerController;
    use crate::{
        events::models::{GameChatMessage, GameEvent, NativeChat},
        game::chivalry2::{AActor, AController, ATBLCharacter, ATBLGameMode, ATBLPlayerState, EChatType},
        ue::{FName, FString},
    };
    use regex::Regex;

    fn has_authority() -> bool {
        crate::globals().world().is_some_and(|world| unsafe {
            !(*world).authority_game_mode.is_null()
        })
    }

    define_pattern_resolver!(ClientReceiveChat, [
        "40 53 55 57 41 57 48 81 EC B8 00 00 00 41 8B E9 4D 8B F8 48 8B DA 48"
    ]);
    CREATE_HOOK!(ClientReceiveChat, (
        receiver: *mut APlayerController, sender: *mut ATBLPlayerState,
        message: *const FString, chat_type: u32, is_sender_dev: bool, override_color: u32
    ), {
        // Hosts capture once at BroadcastChat, not once per local recipient.
        if !has_authority() {
            capture_chat(sender, message, chat_type);
        }
    });

    define_pattern_resolver!(BroadcastChat, [
        "48 85 D2 0F 84 3E 04 00 00 48 8B C4 4C 89 40 18 48 89 48 08 55 41 55 41 56 41 57 48 8D 68 A9 48"
    ]);
    CREATE_HOOK!(BroadcastChat, (
        gamemode: *mut ATBLGameMode, sender: *mut AActor,
        message: *const FString, chat_type: u32, override_color: u32
    ), {
        if !sender.is_null() {
            capture_chat(actor_player_state(sender), message, chat_type);
        }
    });

    fn actor_player_state(sender: *mut AActor) -> *mut ATBLPlayerState {
        use super::super::gameplay::object_inherits_from;
        // BroadcastChat accepts either a pawn or a controller, not a PlayerState.
        if object_inherits_from(sender.cast(), "Pawn") {
            // Read only APawn's field; the sender need not have ATBLCharacter's full layout.
            unsafe { std::ptr::addr_of!((*sender.cast::<ATBLCharacter>())
                .base_tbl_character_base.base_character.base_pawn.player_state).read().cast() }
        } else if object_inherits_from(sender.cast(), "Controller") {
            unsafe { (*sender.cast::<AController>()).player_state.cast() }
        } else {
            std::ptr::null_mut()
        }
    }

    fn capture_chat(sender: *mut ATBLPlayerState, message: *const FString, chat_type: u32) {
        let Some(message) = (unsafe { message.as_ref() }).and_then(|s| s.copy_to_string().ok()) else { return; };
        let Some(chat_type) = u8::try_from(chat_type).ok().and_then(|value| EChatType::try_from(value).ok()) else {
            swarn!("Invalid native chat type: {chat_type}");
            return;
        };
        let Some(player) = (unsafe { sender.as_ref() }) else { return; };
        let Ok(sender) = player.base.player_name_private.copy_to_string() else { return; };
        let command = player_command_event(&sender, &message, chat_type);
        EVENT_SYSTEM.game_event_publisher.publish(GameEvent::GameChatMessageEvent(GameChatMessage {
            sender, message,
            chat_source: ChatSource::Native(NativeChat::Player { channel: chat_type, faction: Some(player.faction) }),
        }));
        if let Some(event) = command {
            EVENT_SYSTEM.game_event_publisher.publish(event);
        }
    }

    define_pattern_resolver!(ClientMessage, [
        "4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 73 18 49 8B D8 49 89 43 C8 48 8B F1 49 89 43 D0 49 89 43 D8 49 8D 43"
    ]);

    // void APlayerController::ClientMessage(const FString& S, FName Type, float MsgLifeTime)
    CREATE_HOOK!(ClientMessage, (this:*mut APlayerController, S:*mut FString, Type:FName, MsgLifeTime: f32), {
        if let Some(message) = (unsafe { S.as_ref() }).and_then(|s| s.copy_to_string().ok()) {
            if let Some((chat_type, sender, message)) = system_message(&message) {
                EVENT_SYSTEM.game_event_publisher.publish(GameEvent::GameChatMessageEvent(GameChatMessage {
                    sender: sender.unwrap_or_default().into(), message: message.into(),
                    chat_source: ChatSource::Native(NativeChat::System { channel: chat_type }),
                }));
            }
        }
    });

    fn player_command_event(sender: &str, message: &str, chat_type: EChatType) -> Option<GameEvent> {
        if chat_type != EChatType::AllSay { return None; }
        let content = message.strip_prefix('!')?;
        let (name, args, raw_args) = parse_command(content)?;
        let actor = CommandActor {
            display_name: sender.into(),
            identity: ActorIdentity::GamePlayer {
                player_id: 0, // TODO
                display_name: sender.into(),
            },
            permissions: ActorPermissions {
                flags: PermissionFlags::USER | PermissionFlags::START_VOTE // TODO: Figure out when this is an admin user
            },
        };
        Some(GameEvent::CommandRequestEvent(CommandRequest {
            name, args, raw_args, actor, source: CommandSource::GameChat,
        }))
    }

    fn system_message(line: &str) -> Option<(EChatType, Option<&str>, &str)> {
        static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
            Regex::new(r"^(?:(?P<player>[^\r\n]*?[ \t]+))??<(?P<channel>[0-9]+)>:[ \t]*").unwrap()
        });
        let (channel, body) = if let Some(caps) = RE.captures(line) {
            // Native hooks own player records; don't parse their channel or body.
            if caps.name("player").is_some() { return None; }
            match caps["channel"].parse::<u8>().ok().and_then(|value| EChatType::try_from(value).ok()) {
                Some(channel) => (channel, &line[caps.get(0).unwrap().end()..]),
                None => (EChatType::ServerSay, line),
            }
        } else {
            (EChatType::ServerSay, line)
        };
        // Console Say carries this visible sender prefix across the native broadcast.
        if matches!(channel, EChatType::AllSay | EChatType::ServerSay) {
            if let Some((title, message)) = crate::events::broadcast_message::game_chat::broadcast_parts(body) {
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
        fn native_player_commands_remain_global_only() {
            assert!(player_command_event("Player", "hello\nworld", EChatType::AllSay).is_none());
            let Some(GameEvent::CommandRequestEvent(command)) = player_command_event("Player", "!vote kick\nreason", EChatType::AllSay) else { panic!("missing command event"); };
            assert_eq!(command.name, "vote");
            assert_eq!(command.args, ["kick", "reason"]);
            assert_eq!(command.raw_args, "kick\nreason");
            for channel in EChatType::ALL.iter().copied().filter(|channel| *channel != EChatType::AllSay) {
                assert!(player_command_event("Player", "!vote kick", channel).is_none());
            }
            assert!(player_command_event("Player", "!", EChatType::AllSay).is_none());
        }

        #[test]
        fn console_named_player_keeps_game_chat_permissions() {
            let Some(GameEvent::CommandRequestEvent(command)) =
                player_command_event("Console", "!cmd quit", EChatType::AllSay)
                else { panic!("missing command event"); };
            assert_eq!(command.source, CommandSource::GameChat);
            assert!(matches!(command.actor.identity, ActorIdentity::GamePlayer { .. }));
            assert_eq!(command.actor.permissions.flags, PermissionFlags::USER | PermissionFlags::START_VOTE);
        }

        #[test]
        fn server_say_preserves_multiline_motd() {
            let message = "Welcome\r\nExample <0>: !not_a_command\nEnjoy!";
            let line = format!("<6>: {message}");
            assert_eq!(system_message(&line), Some((EChatType::ServerSay, None, message)));
        }

        #[test]
        fn player_messages_are_ignored_without_parsing_their_body_or_channel() {
            for line in ["Player Name <0>: [Console] Hello", "Player <1>:", "Player <4294967296>: !vote kick\r\nreason"] {
                assert_eq!(system_message(line), None, "input: {line:?}");
            }
        }

        #[test]
        fn system_headers_preserve_channels_and_unrecognized_text() {
            for (line, channel, body) in [
                ("<0>: Hello", EChatType::AllSay, "Hello"),
                ("<1>:\t \r\nHello", EChatType::TeamSay, "\r\nHello"),
                ("<6>:", EChatType::ServerSay, ""),
                ("<6>:Welcome", EChatType::ServerSay, "Welcome"),
            ] {
                assert_eq!(system_message(line), Some((channel, None, body)));
            }
            for line in ["", "Welcome!", "Welcome\nPlayer <0>: text", "<x>: text", "<256>: Example <0>: text"] {
                assert_eq!(system_message(line), Some((EChatType::ServerSay, None, line)));
            }
        }

        #[test]
        fn console_system_prefix_preserves_sender_and_body() {
            assert_eq!(system_message("<0>: [Console] Hello\r\nworld"),
                Some((EChatType::AllSay, Some("Console"), "Hello\r\nworld")));
        }

        #[test]
        fn broadcast_envelope_is_system_metadata_not_a_player_command() {
            let body = "[Unchained broadcast] Console\n!cmd quit";
            assert_eq!(system_message(&format!("<0>: {body}")), Some((EChatType::AllSay, Some("Console"), "!cmd quit")));
            assert_eq!(system_message(&format!("Player <0>: {body}")), None);
            assert_eq!(system_message(&format!("<1>: {body}")), Some((EChatType::TeamSay, None, body)));
        }
    }
}

// Kismet error messages
#[cfg(feature="kismet_log")]
pub mod kismet_log {
    use crate::ue::{FName, UObject, TCHAR};
    
    // Chiv is spamming these from time to time. Shame, Shame, Shame
    // TODO: Maybe make it dynamic
    static LIST_OF_SHAME: [&str; 4] = [
        "/Game/Maps/Frontend/CIT/FE_Citadel_Atmospherics.FE_Citadel_Atmospherics_C",
        "Divide by zero: ProjectVectorOnToVector with zero Target vector",
        "A null object was passed as a world context object to UEngine::GetWorldFromContextObject().",
        "/Game/Maps/Frontend/Blueprints/Customization_Rotation.Customization_Rotation_C",
    ];

    define_pattern_resolver!(KismetExecutionMessage, [
        "48 89 5C 24 08 57 48 83 EC 30 0F B6 DA 48 8B F9 80 FA 01 ?? ?? ?? ?? ?? ?? ?? ?? ?? BA",
    ]);

    // void __cdecl FFrame::KismetExecutionMessage(wchar_t *param_1,Type param_2,FName param_3)
    CREATE_HOOK!(KismetExecutionMessage, *mut UObject, (Message:*const TCHAR, Type: u8, fname: FName), {
        
        if !Message.is_null() {
            unsafe {
                let msg = widestring::U16CStr::from_ptr_str(Message);
                let mut message = msg.to_string_lossy();
                message = match message.contains('\n') {
                    true => format!("\n{message}"),
                    false => message,
                };
                
                match LIST_OF_SHAME.iter().any(|x| message.contains(x)) {
                    true => {}
                    false => sdebug!("{message}"),
                }
            }
        }

    });
}

define_pattern_resolver!(BroadcastLocalizedChat,["48 89 74 24 10 57 48 83 EC 30 48 8B 01 41 8B F8 48 8B F2 ?? ?? ?? ?? ?? ?? 48 8B C8 48 8D"]);
//void __thiscall ATBLGameMode::BroadcastLocalizedChat(ATBLGameMode *this,FText *param_1,Type param_2)
CREATE_HOOK!(BroadcastLocalizedChat, CALLED, (gamemode: *mut ATBLGameMode, text: *mut FText, chat_type: EChatType),{
});

// Console output (e.g. "Command not recognized: ...")
define_pattern_resolver!(ConsoleOutputDevice__Serialize,["48 89 5C 24 10 48 89 6C 24 18 48 89 74 24 20 41 56 48 83 EC 30 41 0F B6 E8 49 8B D9 44 0F B6 C5 48 8B"]);
// void FConsoleOutputDevice::Serialize(longlong param_1,short *param_2,byte param_3,undefined8 param_4)
CREATE_HOOK!(ConsoleOutputDevice__Serialize,(arg0: *mut FConsoleOutputDevice, arg1: *const TCHAR, arg2: *const u16, arg3: *const u16),{
    unsafe {
        if !arg1.is_null() {
            // get a rust string
            let u16_cstr = widestring::U16CStr::from_ptr_str(arg1);
            let str = u16_cstr.to_string_lossy();
            if !str.starts_with("Command not recognized: RCON_INTERCEPT") {
                sinfo!["{}" , u16_cstr.display()];
            }
        }
    }
});
