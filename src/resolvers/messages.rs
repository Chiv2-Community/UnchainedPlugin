use std::os::raw::c_void;

use crate::{game::{chivalry2::{ATBLGameMode, EChatType}, engine::FText}, ue::FString};


// Chat messages
// #[cfg(feature="client_message")]
mod client_message {
    use log::{info, warn};
    use regex::Regex;
    use std::os::raw::c_void;
    use crate::{events::models::{GameChatMessage, GameEvent}, game::chivalry2::EChatType, ue::{FName, FString}};
    use crate::events::models::{ActorIdentity, ActorPermissions, ChatSource, ChatType, CommandActor, CommandSource, CommandRequest, PermissionFlags};
    use crate::features::events::EVENT_SYSTEM;
    use crate::tools::command_parser::parse_command;

    #[derive(Debug)]
    pub struct ChatMessage<'a> {
        pub name: &'a str,
        pub channel: u32,
        pub message: &'a str,
    }
    
    pub fn parse_chat_line(line: &str) -> Option<ChatMessage<'_>> {
        static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
            // Only the body may span lines; system text must not become a sender.
            Regex::new(r"^([^\r\n]*?)[ \t]+<(\d+)>:[ \t]*((?s:.*))$").unwrap()
        });

        let caps = RE.captures(line)?;

            Some(ChatMessage {
                name: caps.get(1)?.as_str().trim(), // trim to remove trailing space before the middle word
                channel: caps.get(2)?.as_str().parse().ok()?,
                message: caps.get(3)?.as_str(),
            })
    }
    
    fn parse_msg(line: &str) -> Result<Option<(EChatType, &str)>, String> {
        static RE: once_cell::sync::Lazy<Result<Regex, regex::Error>> =
            once_cell::sync::Lazy::new(|| Regex::new(r#"(?s)^<(\d+)>:[ \t]*(.*)$"#));

        // `(?s)` enables dotall so the message capture includes newlines.
        let re = RE
            .as_ref()
            .map_err(|e| format!("parse_msg regex compile failed: {e}"))?;
        // ClientMessage also receives plain server/system text without a chat header.
        let Some(caps) = re.captures(line) else {
            return Ok(None);
        };
        let msg_type_raw = caps
            .get(1)
            .ok_or_else(|| "parse_msg missing chat type capture".to_string())?
            .as_str();
        let msg_type_u8: u8 = msg_type_raw
            .parse()
            .map_err(|e| format!("parse_msg failed to parse chat type '{msg_type_raw}': {e}"))?;
        let msg_type = EChatType::try_from(msg_type_u8)
            .map_err(|_| format!("parse_msg invalid EChatType value: {msg_type_u8}"))?;
        let message = caps
            .get(2)
            .ok_or_else(|| "parse_msg missing message capture".to_string())?
            .as_str();
        Ok(Some((msg_type, message)))
    }

    #[derive(Debug)]
    enum ParsedClientMessage<'a> {
        System(EChatType, &'a str),
        Player(ChatMessage<'a>),
        Plain(&'a str),
    }

    fn parse_client_message(line: &str) -> Result<ParsedClientMessage<'_>, String> {
        // A leading typed header owns the whole message, even if its body looks like player chat.
        // Invalid typed headers must not fall through to player commands either.
        if let Some((chat_type, message)) = parse_msg(line)? {
            return Ok(ParsedClientMessage::System(chat_type, message));
        }
        Ok(match parse_chat_line(line) {
            Some(chat) => ParsedClientMessage::Player(chat),
            None => ParsedClientMessage::Plain(line),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn plain_motd_has_no_chat_header() {
            for message in ["Welcome to the server!", "Welcome\nRead the rules.", ""] {
                assert!(parse_chat_line(message).is_none());
                assert_eq!(parse_msg(message).unwrap(), None);
            }
        }

        #[test]
        fn server_say_preserves_multiline_motd() {
            let message = "Welcome\r\nRead the rules.\nEnjoy!";
            let line = format!("<6>: {message}");
            assert_eq!(parse_msg(&line).unwrap(), Some((EChatType::ServerSay, message)));
        }

        #[test]
        fn system_messages_allow_empty_text_and_no_separator_space() {
            assert_eq!(parse_msg("<6>:").unwrap(), Some((EChatType::ServerSay, "")));
            assert_eq!(parse_msg("<6>:Welcome").unwrap(), Some((EChatType::ServerSay, "Welcome")));
        }

        #[test]
        fn header_like_text_inside_motd_is_not_a_header() {
            assert_eq!(parse_msg("Welcome! Example: <99>: text").unwrap(), None);
        }

        #[test]
        fn typed_multiline_system_text_never_matches_player_chat() {
            for newline in ["\n", "\r\n", "\r"] {
                for channel in ["0", "99", "256", "4294967296"] {
                    for text in ["text", "!help"] {
                        let message = format!("Welcome{newline}Example <{channel}>: {text}");
                        let line = format!("<6>: {message}");
                        assert!(parse_chat_line(&line).is_none(), "input: {line:?}");
                        assert_eq!(parse_msg(&line).unwrap(), Some((EChatType::ServerSay, message.as_str())));
                    }
                }
            }
        }

        #[test]
        fn untyped_multiline_system_text_never_matches_player_chat() {
            for newline in ["\n", "\r\n", "\r"] {
                for channel in ["0", "99", "256", "4294967296"] {
                    for text in ["text", "!help"] {
                        for sender in ["Example ", ""] {
                            let line = format!("Welcome{newline}{sender}<{channel}>: {text}");
                            assert!(parse_chat_line(&line).is_none(), "input: {line:?}");
                            assert_eq!(parse_msg(&line).unwrap(), None);
                        }
                    }
                }
            }
        }

        #[test]
        fn invalid_chat_types_still_report_errors() {
            assert!(parse_msg("<99>: text").is_err());
            assert!(parse_msg("<256>: text").is_err());
        }

        #[test]
        fn player_chat_preserves_sender_channel_and_multiline_text() {
            let chat = parse_chat_line("Player Name <0>: Hello\r\nworld").unwrap();
            assert_eq!(chat.name, "Player Name");
            assert_eq!(chat.channel, 0);
            assert_eq!(chat.message, "Hello\r\nworld");
        }

        #[test]
        fn multiline_commands_keep_command_prefix() {
            let chat = parse_chat_line("Player <0>: !vote kick\nreason").unwrap();
            assert_eq!(chat.name, "Player");
            assert_eq!(chat.channel, 0);
            assert_eq!(chat.message, "!vote kick\nreason");
            assert_eq!(parse_command(&chat.message[1..]).unwrap(), (
                "vote".to_owned(), vec!["kick".to_owned(), "reason".to_owned()], "kick\nreason".to_owned(),
            ));
        }

        #[test]
        fn routing_gives_leading_typed_headers_precedence_over_embedded_player_headers() {
            for newline in ["\n", "\r\n", "\r"] {
                for channel in ["0", "99", "256", "4294967296"] {
                    for text in ["text", "!help"] {
                        for body in [
                            format!("Example <{channel}>: {text}{newline}Read the rules."),
                            format!("Welcome{newline}Example <{channel}>: {text}"),
                            format!("Example <{channel}>: {text}"),
                        ] {
                            let line = format!("<6>: {body}");
                            match parse_client_message(&line).unwrap() {
                                ParsedClientMessage::System(chat_type, message) => {
                                    assert_eq!(chat_type, EChatType::ServerSay);
                                    assert_eq!(message, body);
                                }
                                other => panic!("input: {line:?}, route: {other:?}"),
                            }
                        }
                    }
                }
            }
        }

        #[test]
        fn routing_rejects_invalid_leading_headers_without_player_fallback() {
            for outer_channel in ["99", "256", "4294967296"] {
                for newline in ["\n", "\r\n", "\r"] {
                    for channel in ["0", "99", "256", "4294967296"] {
                        for text in ["text", "!help"] {
                            for body in [
                                format!("Example <{channel}>: {text}{newline}Read the rules."),
                                format!("Welcome{newline}Example <{channel}>: {text}"),
                                format!("Example <{channel}>: {text}"),
                            ] {
                                let line = format!("<{outer_channel}>: {body}");
                                assert!(parse_client_message(&line).is_err(), "input: {line:?}");
                            }
                        }
                    }
                }
            }
        }

        #[test]
        fn routing_preserves_plain_and_empty_system_text() {
            for line in ["", "Welcome!", "Welcome\nRead the rules."] {
                assert!(matches!(parse_client_message(line).unwrap(),
                    ParsedClientMessage::Plain(message) if message == line));
            }
            for (line, body) in [("<6>:", ""), ("<6>:Welcome", "Welcome")] {
                assert!(matches!(parse_client_message(line).unwrap(),
                    ParsedClientMessage::System(EChatType::ServerSay, message) if message == body));
            }
            for newline in ["\n", "\r\n", "\r"] {
                for channel in ["0", "99", "256", "4294967296"] {
                    for text in ["text", "!help"] {
                        for sender in ["Example ", ""] {
                            let line = format!("Welcome{newline}{sender}<{channel}>: {text}");
                            assert!(matches!(parse_client_message(&line).unwrap(),
                                ParsedClientMessage::Plain(message) if message == line));
                        }
                    }
                }
            }
        }

        #[test]
        fn routing_preserves_multiline_player_chat_and_commands() {
            for newline in ["\n", "\r\n", "\r"] {
                for body in [
                    format!("Hello{newline}world"),
                    format!("!vote kick{newline}reason"),
                    format!("Example <6>: text{newline}<99>: !help"),
                ] {
                    let line = format!("Player Name <0>: {body}");
                    let ParsedClientMessage::Player(chat) = parse_client_message(&line).unwrap() else {
                        panic!("player message routed as system text: {line:?}");
                    };
                    assert_eq!(chat.name, "Player Name");
                    assert_eq!(chat.channel, 0);
                    assert_eq!(chat.message, body);
                    if let Some(command) = chat.message.strip_prefix('!') {
                        let (name, _, raw_args) = parse_command(command).unwrap();
                        assert_eq!(name, "vote");
                        assert_eq!(raw_args, format!("kick{newline}reason"));
                    }
                }
            }
        }
    }

    
    define_pattern_resolver!(ClientMessage, [
        "4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 73 18 49 8B D8 49 89 43 C8 48 8B F1 49 89 43 D0 49 89 43 D8 49 8D 43"
    ]);

    // void APlayerController::ClientMessage(const FString& S, FName Type, float MsgLifeTime)
    CREATE_HOOK!(ClientMessage, (this:*mut c_void, S:*mut FString, Type:FName, MsgLifeTime: f32), {
        let string_ref: &FString = unsafe{ &*S };
        let message = string_ref.to_string();
        match parse_client_message(message.as_str()) {
            Ok(ParsedClientMessage::Player(chat)) => {
                match chat.message.starts_with('!') {
                    true => {
                        let msg_type = EChatType::try_from(chat.channel as u8).expect("Failed to parse EChatType");
                        info!(target: "game_chat", "\x1b[38;5;214m[ {:10?} ] \x1b[38;5;251m[ {} ]\x1b[38;5;255m: \x1b[38;5;251m{}\x1b[38;5;255m", msg_type, chat.name, chat.message);

                        if msg_type == EChatType::AllSay {
                            let content = &chat.message[1..];
                            if let Some((name, args, raw_args)) = parse_command(content) {
                                let actor = CommandActor {
                                    display_name: chat.name.into(),
                                    identity: ActorIdentity::GamePlayer {
                                        player_id: 0, // TODO
                                        display_name: chat.name.into(),
                                    },
                                    permissions: ActorPermissions {
                                        flags: PermissionFlags::USER | PermissionFlags::START_VOTE // TODO: Figure out when this is an admin user
                                    },
                                };
                                EVENT_SYSTEM.game_event_publisher.publish(GameEvent::CommandRequestEvent(CommandRequest {
                                    name,
                                    args,
                                    raw_args,
                                    actor,
                                    source: CommandSource::GameChat,
                                }));
                            }
                        }
                    }
                    false => {
                        let msg_type = EChatType::try_from(chat.channel as u8).expect("Failed to parse EChatType");
                        info!(target: "game_chat", "\x1b[38;5;214m[ {:10?} ] \x1b[38;5;251m[ {} ]\x1b[38;5;255m: \x1b[38;5;251m{}\x1b[38;5;255m", msg_type, chat.name, chat.message);
                        
                        if msg_type == EChatType::AllSay {
                            EVENT_SYSTEM.game_event_publisher.publish(
                                GameEvent::GameChatMessageEvent(GameChatMessage {
                                    sender: chat.name.into(),
                                    message: chat.message.into(),
                                    chat_type: ChatType::Global,
                                    chat_source: ChatSource::Game,
                                })
                            );
                        }
                    }
                };        
            }
            Ok(ParsedClientMessage::System(chat_type, message)) => {
                info!(target: "system_chat", "\x1b[38;5;214m[ {chat_type:10?} ] \x1b[38;5;251m{message}\x1b[38;5;255m");
            }
            Ok(ParsedClientMessage::Plain(message)) => {
                info!(target: "system_chat", "{message}");
            }
            Err(error) => {
                warn!(target: "system_chat", "Failed to parse system chat message. error={error}");
            }
        }
    });
}

// Kismet error messages
#[cfg(feature="kismet_log")]
pub mod kismet_log {
    use crate::ue::{FName, UObject};
    
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
    CREATE_HOOK!(KismetExecutionMessage, *mut UObject, (Message:*const u16, Type: u8, fname: FName), {
        
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
                    false => log::debug!(target: "kismet", "{message}"),
                }
            }
        }

    });
}

// /*
// void ATBLPlayerController::ClientReceiveChat_Implementation(ATBLPlayerState* SenderPlayerState, const FString& S, TEnumAsByte<EChatType::Type> Type, bool IsSenderDev, FColor OverrideColor) {
// }
// */
// DECL_HOOK(void, ClientReceiveChat_Implementation, (/*ATBLPlayerState * */void* SenderPlayerState, FString* S, /*TEnumAsByte<EChatType::Type>*/void * Type, bool IsSenderDev, /*FColor*/void * OverrideColor))
// {
// 	/*std::wcout << "crc: " << *S->str << std::endl;
// 	log("ClientReceiveChat_Implementation");
// 	logWideString(S->str);*/
// 	o_ClientReceiveChat_Implementation(SenderPlayerState, S, Type, IsSenderDev, OverrideColor);
// }

// TODO: Unused
// define_pattern_resolver!(execBroadcastLocalizedChat,["48 89 5C 24 08 57 48 83 EC 60 48 8D 4C 24 28 48 8B DA ?? ?? ?? ?? ?? 48 83 7B 20 00 48 8B"]);
// // UTBLOnlineLibrary::execBroadcastLocalizedChat(UObject *param_1,FFrame *param_2,void *param_3)
// CREATE_HOOK!(execBroadcastLocalizedChat, INACTIVE, (msg: *mut FString, arg2: *mut c_void, arg3: *mut c_void),{
//     if !msg.is_null() {
//         let url_w = unsafe { (*msg).to_string() };
//         crate::sinfo![f; "Triggered! {url_w}"];
//     }
// });

define_pattern_resolver!(BroadcastLocalizedChat,["48 89 74 24 10 57 48 83 EC 30 48 8B 01 41 8B F8 48 8B F2 ?? ?? ?? ?? ?? ?? 48 8B C8 48 8D"]);
//void __thiscall ATBLGameMode::BroadcastLocalizedChat(ATBLGameMode *this,FText *param_1,Type param_2)
CREATE_HOOK!(BroadcastLocalizedChat, CALLED, (gamemode: *mut ATBLGameMode, text: *mut FText, chat_type: EChatType),{
    crate::sinfo![f; "Triggered!"];
});

// TODO: Not used?
define_pattern_resolver!(ClientReceiveChat_Implementation,["40 53 55 57 41 57 48 81 EC B8 00 00 00 41 8B E9 4D 8B F8 48 8B DA 48"]);
// void ATBLPlayerController::ClientReceiveChat_Implementation(ATBLPlayerState* SenderPlayerState, const FString& S, TEnumAsByte<EChatType::Type> Type, bool IsSenderDev, FColor OverrideColor) {
CREATE_HOOK!(ClientReceiveChat_Implementation, INACTIVE, (SenderPlayerState: *mut c_void, S: *mut FString, Type: *mut c_void, IsSenderDev: bool, OverrideColor: *mut c_void),{
    if !S.is_null() {
        let url_w = unsafe { (*S).to_string() };
        crate::sinfo![f; "Triggered! {url_w}"];
    }
});

// Console output (e.g. "Command not recognized: ...")
define_pattern_resolver!(ConsoleOutputDevice__Serialize,["48 89 5C 24 10 48 89 6C 24 18 48 89 74 24 20 41 56 48 83 EC 30 41 0F B6 E8 49 8B D9 44 0F B6 C5 48 8B"]);
// void FConsoleOutputDevice::Serialize(longlong param_1,short *param_2,byte param_3,undefined8 param_4)
CREATE_HOOK!(ConsoleOutputDevice__Serialize,(arg0: *mut c_void, arg1: *const u16, arg2: *const u16, arg3: *const u16),{
    unsafe {
        if !arg1.is_null() {
            // get a rust string
            let u16_cstr = widestring::U16CStr::from_ptr_str(arg1);
            let str = u16_cstr.to_string_lossy();
            if !str.starts_with("Command not recognized: RCON_INTERCEPT") {
                log::info![target: "Console", "{}" , u16_cstr.display()];
            }
        }
    }
});
