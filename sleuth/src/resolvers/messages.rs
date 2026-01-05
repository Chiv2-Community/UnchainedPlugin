

// Chat messages
// #[cfg(feature="client_message")]
mod client_message {
    use std::str::FromStr;
    use log::info;
    use regex::Regex;
    use std::os::raw::c_void;
    use crate::{sinfo, ue::{FName, FString}};

    // Chat type enum
    // FIXME: More compact, wtf is this
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[allow(clippy::upper_case_acronyms)]
    pub enum EChatType {
        AllSay,
        TeamSay,
        Whisper,
        Admin,
        Objective,
        System,
        ServerSay,
        Debug,
        CrosshairMsg,
        Backend,
        Party,
        Spectator,
        ClosedCaption,
        ClosedCaptionMason,
        ClosedCaptionAgatha,
        MAX,
    }

    impl FromStr for EChatType {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "AllSay" => Ok(EChatType::AllSay),
                "TeamSay" => Ok(EChatType::TeamSay),
                "Whisper" => Ok(EChatType::Whisper),
                "Admin" => Ok(EChatType::Admin),
                "Objective" => Ok(EChatType::Objective),
                "System" => Ok(EChatType::System),
                "ServerSay" => Ok(EChatType::ServerSay),
                "Debug" => Ok(EChatType::Debug),
                "CrosshairMsg" => Ok(EChatType::CrosshairMsg),
                "Backend" => Ok(EChatType::Backend),
                "Party" => Ok(EChatType::Party),
                "Spectator" => Ok(EChatType::Spectator),
                "ClosedCaption" => Ok(EChatType::ClosedCaption),
                "ClosedCaptionMason" => Ok(EChatType::ClosedCaptionMason),
                "ClosedCaptionAgatha" => Ok(EChatType::ClosedCaptionAgatha),
                "MAX" => Ok(EChatType::MAX),
                _ => Err(()),
            }
        }
    }

    impl TryFrom<u8> for EChatType {
        type Error = ();

        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                0 => Ok(EChatType::AllSay),
                1 => Ok(EChatType::TeamSay),
                2 => Ok(EChatType::Whisper),
                3 => Ok(EChatType::Admin),
                4 => Ok(EChatType::Objective),
                5 => Ok(EChatType::System),
                6 => Ok(EChatType::ServerSay),
                7 => Ok(EChatType::Debug),
                8 => Ok(EChatType::CrosshairMsg),
                9 => Ok(EChatType::Backend),
                10 => Ok(EChatType::Party),
                11 => Ok(EChatType::Spectator),
                12 => Ok(EChatType::ClosedCaption),
                13 => Ok(EChatType::ClosedCaptionMason),
                14 => Ok(EChatType::ClosedCaptionAgatha),
                15 => Ok(EChatType::MAX),
                _ => Err(()),
            }
        }
    }

    #[derive(Debug)]
    pub struct ChatMessage<'a> {
        pub name: &'a str,
        pub channel: u32,
        pub message: &'a str,
    }
    
    pub fn parse_chat_line(line: &str) -> Option<ChatMessage> {
        static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
            Regex::new(r"^(\w+)\s+\w+\s+<(\d+)>:\s+(.*)$").unwrap()
        });
    
        RE.captures(line).map(|caps| ChatMessage {
            name: caps.get(1).unwrap().as_str(),
            channel: caps.get(2).unwrap().as_str().parse().ok().unwrap(),
            message: caps.get(3).unwrap().as_str(),
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
