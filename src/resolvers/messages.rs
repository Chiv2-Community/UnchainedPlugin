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

    #[derive(Debug, PartialEq)]
    pub struct ChatMessage<'a> {
        pub sender: Option<&'a str>,
        pub chat_type: Option<EChatType>,
        pub message: &'a str,
    }

    fn parse_client_message(line: &str) -> Result<ChatMessage<'_>, String> {
        static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
            Regex::new(concat!(
                r"^",                                 // Start of input.
                r"(?:(?P<sender>[^\r\n]*?)[ \t]+)??", // Optional single-line sender; prefer no sender.
                r"<(?P<chat_type>\d+)>:",             // Numeric chat type header.
                r"[ \t]*",                            // Skip horizontal whitespace before the body.
                r"(?P<message>(?s:.*))",              // Body, including newlines or empty text.
                r"$",                                 // End of input.
            )).unwrap()
        });

        let Some(caps) = RE.captures(line) else {
            return Ok(ChatMessage { sender: None, chat_type: None, message: line });
        };
        // Validate after matching so invalid leading headers cannot fall back to player chat.
        let value = caps.name("chat_type").unwrap().as_str();
        let value: u8 = value.parse()
            .map_err(|error| format!("Invalid chat type '{value}': {error}"))?;
        let chat_type = EChatType::try_from(value)
            .map_err(|_| format!("Invalid EChatType value: {value}"))?;
        Ok(ChatMessage {
            sender: caps.name("sender").map(|sender| sender.as_str().trim()),
            chat_type: Some(chat_type),
            message: caps.name("message").unwrap().as_str(),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn server_say_preserves_multiline_motd() {
            let message = "Welcome\r\nRead the rules.\nEnjoy!";
            let line = format!("<6>: {message}");
            assert_eq!(parse_client_message(&line).unwrap(), ChatMessage {
                sender: None, chat_type: Some(EChatType::ServerSay), message,
            });
        }

        #[test]
        fn player_messages_distinguish_all_chat_from_team_chat() {
            for body in ["Hello", "", "!help", "Hello\nworld", "!vote kick\r\nreason"] {
                for (channel, chat_type) in [(0, EChatType::AllSay), (1, EChatType::TeamSay)] {
                    let line = format!("Player Name <{channel}>: {body}");
                    assert_eq!(parse_client_message(&line).unwrap(), ChatMessage {
                        sender: Some("Player Name"), chat_type: Some(chat_type), message: body,
                    }, "input: {line:?}");
                }
            }
        }

        #[test]
        fn senderless_messages_distinguish_all_chat_from_team_chat() {
            for (line, chat_type) in [("<0>: Hello", EChatType::AllSay), ("<1>: Hello", EChatType::TeamSay)] {
                assert_eq!(parse_client_message(line).unwrap(), ChatMessage {
                    sender: None, chat_type: Some(chat_type), message: "Hello",
                }, "input: {line:?}");
            }
        }

        #[test]
        fn chat_types_are_validated_without_truncation_or_panics() {
            for channel in ["16", "99", "256", "257", "4294967296"] {
                for message in ["text", "!help", "!help\nreason"] {
                    for prefix in ["", "Player "] {
                        let line = format!("{prefix}<{channel}>: {message}");
                        assert!(parse_client_message(&line).is_err(), "input: {line:?}");
                    }
                }
            }
            for chat_type in EChatType::ALL {
                for (prefix, sender) in [("", None), ("Player ", Some("Player"))] {
                    let line = format!("{prefix}<{}>: text", chat_type as u8);
                    assert_eq!(parse_client_message(&line).unwrap(), ChatMessage {
                        sender, chat_type: Some(chat_type), message: "text",
                    });
                }
            }
        }

        #[test]
        fn horizontal_separators_trim_sender_but_preserve_body() {
            for (line, sender, message) in [
                ("  Player Name \t<0>:\t Hello \t", Some("Player Name"), "Hello \t"),
                ("\t<0>:Hello", Some(""), "Hello"),
                ("Player <0>:", Some("Player"), ""),
                ("<0>:\t \r\nHello", None, "\r\nHello"),
                ("Player <0>:\n Hello", Some("Player"), "\n Hello"),
            ] {
                assert_eq!(parse_client_message(line).unwrap(), ChatMessage {
                    sender, chat_type: Some(EChatType::AllSay), message,
                });
            }
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
                            assert_eq!(parse_client_message(&line).unwrap(), ChatMessage {
                                sender: None, chat_type: Some(EChatType::ServerSay), message: &body,
                            }, "input: {line:?}");
                        }
                    }
                }
            }
        }

        #[test]
        fn routing_rejects_invalid_leading_headers_without_player_fallback() {
            for outer_channel in ["16", "99", "256", "257", "4294967296"] {
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
            for line in ["", "Welcome!", "Welcome\nRead the rules.", "Unknown <x>: text", "<6> text"] {
                assert_eq!(parse_client_message(line).unwrap(), ChatMessage {
                    sender: None, chat_type: None, message: line,
                });
            }
            for (line, body) in [("<6>:", ""), ("<6>:Welcome", "Welcome")] {
                assert_eq!(parse_client_message(line).unwrap(), ChatMessage {
                    sender: None, chat_type: Some(EChatType::ServerSay), message: body,
                });
            }
            for newline in ["\n", "\r\n", "\r"] {
                for channel in ["0", "99", "256", "4294967296"] {
                    for text in ["text", "!help"] {
                        for sender in ["Example ", ""] {
                            let line = format!("Welcome{newline}{sender}<{channel}>: {text}");
                            assert_eq!(parse_client_message(&line).unwrap(), ChatMessage {
                                sender: None, chat_type: None, message: &line,
                            });
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
                    let chat = parse_client_message(&line).unwrap();
                    assert_eq!(chat.sender, Some("Player Name"));
                    assert_eq!(chat.chat_type, Some(EChatType::AllSay));
                    assert_eq!(chat.message, body);
                    if let Some(command) = chat.message.strip_prefix('!') {
                        let (name, args, raw_args) = parse_command(command).unwrap();
                        assert_eq!(name, "vote");
                        if newline == "\n" {
                            assert_eq!(args, ["kick", "reason"]);
                        }
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
            Ok(ChatMessage { sender: Some(sender), chat_type: Some(chat_type), message }) => {
                info!(target: "game_chat", "\x1b[38;5;214m[ {chat_type:10?} ] \x1b[38;5;251m[ {sender} ]\x1b[38;5;255m: \x1b[38;5;251m{message}\x1b[38;5;255m");
                if chat_type == EChatType::AllSay {
                    if let Some(content) = message.strip_prefix('!') {
                        if let Some((name, args, raw_args)) = parse_command(content) {
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
                            EVENT_SYSTEM.game_event_publisher.publish(GameEvent::CommandRequestEvent(CommandRequest {
                                name,
                                args,
                                raw_args,
                                actor,
                                source: CommandSource::GameChat,
                            }));
                        }
                    } else {
                        EVENT_SYSTEM.game_event_publisher.publish(GameEvent::GameChatMessageEvent(GameChatMessage {
                            sender: sender.into(),
                            message: message.into(),
                            chat_type: ChatType::Global,
                            chat_source: ChatSource::Game,
                        }));
                    }
                }
            }
            Ok(ChatMessage { chat_type: Some(chat_type), message, .. }) => {
                info!(target: "system_chat", "\x1b[38;5;214m[ {chat_type:10?} ] \x1b[38;5;251m{message}\x1b[38;5;255m");
            }
            Ok(ChatMessage { message, .. }) => {
                info!(target: "system_chat", "{message}");
            }
            Err(error) => {
                warn!(target: "system_chat", "Failed to parse chat message. error={error}");
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
