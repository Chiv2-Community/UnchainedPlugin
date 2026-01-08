use std::os::raw::c_void;

use crate::{game::{chivalry2::EChatType, engine::FText}, ue::FString};
use crate::globals;


// Chat messages
// #[cfg(feature="client_message")]
mod client_message {
    use log::info;
    use regex::Regex;
    use std::os::raw::c_void;
    use crate::{game::chivalry2::EChatType, tools::hook_globals::globals, ue::{FName, FString}};

    #[derive(Debug)]
    pub struct ChatMessage<'a> {
        pub name: &'a str,
        pub channel: u32,
        pub message: &'a str,
    }
    
    pub fn parse_chat_line(line: &str) -> Option<ChatMessage<'_>> {
        static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
            Regex::new(r"^(.*?)\s+<(\d+)>:\s+(.*)$").unwrap()
        });

        let caps = RE.captures(line)?;

            Some(ChatMessage {
                name: caps.get(1)?.as_str().trim(), // trim to remove trailing space before the middle word
                channel: caps.get(2)?.as_str().parse().ok()?,
                message: caps.get(3)?.as_str(),
            })
    }
    
    pub fn parse_msg_line(line: &str) -> Option<(EChatType, &str)> {
        let re = Regex::new(r#"<(\d+)>: (.+)"#).unwrap();

        if let Some(caps) = re.captures(line) {
            let msg_type_str: u8 = caps.get(1)?.as_str().parse().ok()?;
            let msg_type = EChatType::try_from(msg_type_str).expect("Failed to parse EChatType");
            let message = caps.get(2)?.as_str();
            Some((msg_type, message))
        } else {
            None
        }
    }
    define_pattern_resolver!(ClientMessage, [
        "4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 73 18 49 8B D8 49 89 43 C8 48 8B F1 49 89 43 D0 49 89 43 D8 49 8D 43"
    ]);

    CREATE_HOOK!(ClientMessage, (this:*mut c_void, S:*mut FString, Type:FName, MsgLifeTime: f32), {
        let string_ref: &FString = unsafe{ &*S };
        let message = string_ref.to_string();
        let cmd_filter = |c| ['/', '.'].contains(&c);
        let message_repl = match message.contains('\n') {
            // TODO: Decide what to do with multi line text
            true => format!("\n{message}").replace("\r\n", "\\n"),
            false => message,
        };
        match parse_chat_line(message_repl.as_str()) {
            Some(chat) => {
                match chat.message.starts_with(cmd_filter) {
                    true => {
                        // TODO: handle console commands
                        // Is checking by name sufficient?
                    }
                    false => {
                        let msg_type = EChatType::try_from(chat.channel as u8).expect("Failed to parse EChatType");
                        info!(target: "game_chat", "\x1b[38;5;214m[ {:10?} ] \x1b[38;5;251m[ {} ]\x1b[38;5;255m: \x1b[38;5;251m{}\x1b[38;5;255m", msg_type, chat.name, chat.message);
                        
                        #[cfg(feature="discord_integration")]
                        {
                            if (msg_type == EChatType::AllSay && globals().cli_args.rcon_port.is_some()) {
                                
                                crate::sinfo!(f; "pre Sending message to discord");
                                if let Some(bridge) = globals().DISCORD_BRIDGE.get() {
                                    bridge.recv_game_message(msg_type, chat.name, chat.message);
                                }
                            }
                        }
                    }
                };        
            }
            _ => {
                if let Some((chat_type, message)) = parse_msg_line(message_repl.as_str()) {
                    if let Some(msg) = parse_chat_line(message_repl.as_str()) {
                        info!("Chat: channel {}, name {}, message {}, ", msg.channel, msg.name, msg.message);
                    }
                    else {
                        info!(target: "system_chat", "\x1b[38;5;214m[ {chat_type:10?} ] \x1b[38;5;251m{message}\x1b[38;5;255m");
                    }
                }
                else {
                    println!("something went wrong");
                }
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
// * void __cdecl
// UTBLOnlineLibrary::execBroadcastLocalizedChat(UObject *param_1,FFrame *param_2,void *param_3)
// */
// DECL_HOOK(void, execBroadcastLocalizedChat, (FString* msg, void* param_2, void* param_3))
// {
// 	/*log("execBroadcastLocalizedChat");
// 	logWideString(msg->str);*/
// 	o_execBroadcastLocalizedChat(msg, param_2, param_3);
// }


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

define_pattern_resolver!(execBroadcastLocalizedChat,["48 89 5C 24 08 57 48 83 EC 60 48 8D 4C 24 28 48 8B DA ?? ?? ?? ?? ?? 48 83 7B 20 00 48 8B"]);
// UTBLOnlineLibrary::execBroadcastLocalizedChat(UObject *param_1,FFrame *param_2,void *param_3)
CREATE_HOOK!(execBroadcastLocalizedChat, INACTIVE, (msg: *mut FString, arg2: *mut c_void, arg3: *mut c_void),{
    if !msg.is_null() {
        let url_w = unsafe { (*msg).to_string() };
        crate::sinfo![f; "Triggered! {url_w}"];
    }
});

define_pattern_resolver!(BroadcastLocalizedChat,["48 89 74 24 10 57 48 83 EC 30 48 8B 01 41 8B F8 48 8B F2 ? ? ? ? ? ? 48 8B C8 48 8D"]);
//void __thiscall ATBLGameMode::BroadcastLocalizedChat(ATBLGameMode *this,FText *param_1,Type param_2)
CREATE_HOOK!(BroadcastLocalizedChat, INACTIVE, (gamemode: *mut c_void, text: *mut FText, chat_type: EChatType),{
    crate::sinfo![f; "Triggered!"];
});


define_pattern_resolver!(ClientReceiveChat_Implementation,["40 53 55 57 41 57 48 81 EC B8 00 00 00 41 8B E9 4D 8B F8 48 8B DA 48"]);
// void ATBLPlayerController::ClientReceiveChat_Implementation(ATBLPlayerState* SenderPlayerState, const FString& S, TEnumAsByte<EChatType::Type> Type, bool IsSenderDev, FColor OverrideColor) {
CREATE_HOOK!(ClientReceiveChat_Implementation, INACTIVE, (SenderPlayerState: *mut c_void, S: *mut FString, Type: *mut c_void, IsSenderDev: bool, OverrideColor: *mut c_void),{
    if !S.is_null() {
        let url_w = unsafe { (*S).to_string() };
        crate::sinfo![f; "Triggered! {url_w}"];
    }
});

	/* UTBLOnlineLibrary::execBroadcastLocalizedChat */
	// "48 89 5C 24 08 57 48 83 EC 60 48 8D 4C 24 28 48 8B DA ? ? ? ? ? 48 83 7B 20 00 48 8B",
	/*ClientReceiveChat_Implementation*/
	// "40 53 55 57 41 57 48 81 EC B8 00 00 00 41 8B E9 4D 8B F8 48 8B DA 48",


