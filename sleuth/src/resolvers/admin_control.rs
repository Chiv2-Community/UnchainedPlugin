

use client_message::{parse_chat_line, ChatMessage};
use log::{debug, info, warn};
use retour::static_detour;
use std::{collections::HashMap, sync::Arc};
use std::error::Error;
use std::os::raw::c_void;
use std::mem;
use crate::resolvers::rcon::LAST_COMMAND;
use crate::chiv2::*;
use crate::{globals, ue::*};

define_pattern_resolver![UTBLLocalPlayer_Exec, {
    // "75 18 ?? ?? ?? ?? 75 12 4d 85 f6 74 0d 41 38 be ?? ?? ?? ?? 74 04 32 db eb 9b 48 8b 5d 7f 49 8b d5 4c 8b 45 77 4c 8b cb 49 8b cf", // EGS - latest
    // "75 17 45 84 ED", // STEAM
    // From Sigga
    OTHER: ["75 ?? 45 84 ed 75 ?? 48 85 f6 74 ?? 40 38 be ?? 01 00 00"], // PDB + STEAM
    EGS: ["75 18 40 38 7d d7 75 12 4d 85 f6 74 0d 41 38 be ?? 01 00 00"], // EGS
    // "75 1a 45 84 ed 75 15 48 85 f6 74 10 40 38 be b0 01 00 00 74 07 32 db e9 a6 fd ff ff 48 8b 5d 60 49 8b d6 4c 8b 45 58 4c 8b cb 49 8b cf", // PDB
}];

define_pattern_resolver!(ExecuteConsoleCommand, [
    "40 53 48 83 EC 30 48 8B 05 ?? ?? ?? ?? 48 8B D9 48 8B 90 58 0C 00 00"
]);

// FIXME: Nihi: stub
CREATE_HOOK!(ExecuteConsoleCommand, (string:*mut FString), {
    println!("ExecuteConsoleCommand: {}", unsafe { &*string });
});
CREATE_HOOK!(SomeRandomFunction, (string:*mut FString), {
    println!("SomeRandomFunction: {}", unsafe { &*string });
});

// Executes pending RCON command
CREATE_HOOK!(UGameEngineTick, (engine:*mut c_void, delta:f32, state:u8), {
    let lock = Arc::clone(&crate::resolvers::rcon::LAST_COMMAND);
    let mut fstring = FString::default();
    if let Some(cmd) = crate::resolvers::rcon::LAST_COMMAND.lock().unwrap().as_ref() {
        fstring = FString::from(
            widestring::U16CString::from_str(cmd.as_str())
            .unwrap()
            .as_slice_with_nul());
    }

    if fstring.len() > 1 {
        warn!("Execuing Command: {}", fstring);
        *lock.lock().unwrap() = None;
        unsafe { o_ExecuteConsoleCommand.call(&mut fstring); }
    }
});

// FIXME: Nihi: stub
CREATE_HOOK!(FEngineLoopInit, (engine_loop:*mut c_void), {
// println!("Engine Loop initialized!!");
});

// FText* __cdecl FText::AsCultureInvariant(FText* __return_storage_ptr__, FString* param_1)
define_pattern_resolver![FText_AsCultureInvariant,  First, {
    EGS: ["48 89 5C 24 18 48 89 74 24 20 41 56 48 83 EC 60 33 C0 48 89 7C 24 78 48 63"],
    STEAM: ["40 53 55 57 48 83 EC 50 83 7A 08 01 48 8B F9 4C 89 B4 24 80 00 00 00 C7 44 24 70 00 00 00 00 7F 33 E8 ?? ?? ?? ?? 48 8B 58 08 48 8B 08 48 89 4C 24 20 48 89 5C 24 28 48 85 DB 74 04 F0 FF 43 08 8B 40 10 41 BE 01 00 00 00 89 44 24 30 48 8D 44 24 20 EB 18 48 8D 4C 24 38 E8 ?? ?? ?? ?? 48 8B 5C 24 28 41 BE 02 00 00 00 48 8B 08 48 89 0F 48 8B 48 08 48 89 4F 08 48 85 C9 74 04 F0 FF 41 08 8B 40 10 BD FF FF FF FF 89 47 10 41 F6 C6 02 74 46 48 89 74 24 78 41 83 E6 FD 48 8B 74 24 40 48 85 F6 74 2E 8B C5 F0 0F C1 46 08 83 F8 01 75 22 48 8B 06 48 8B CE FF 10 8B C5 F0 0F C1 46 0C 83 F8 01 75 0E 48 8B 06 BA 01 00 00 00 48 8B CE FF 50 ?? 48 8B 74 24 78 41 F6 C6 01 4C 8B B4 24 80 00 00 00 74 2E 48 85 DB 74 29 8B C5 F0 0F C1 43 08 83 F8 01 75 1D 48 8B 03 48 8B CB FF 10 F0 0F C1 6B 0C 83 FD 01 75 0B 48 8B 03 8B D5 48 8B CB FF 50 ?? 83 4F 10 02"]
}
// ,|ctx, patterns| {
//     let futures = ::patternsleuth::resolvers::futures::future::join_all(
//         patterns.iter()
//             .map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
//     ).await;

//     futures.into_iter().flatten().collect::<Vec<usize>>()[0]
// }
];


// define_pattern_resolver!(ConsoleCommand, First, [
//     "40 53 48 83 EC 20 48 8B 89 D0 02 00 00 48 8B DA 48 85 C9 74 0E E8 ?? ?? ?? ?? 48 8B C3 48 83 C4 20 5B C3 33 C0 48 89 02 48 89 42 08 48 8B C3 48 83 C4 20 5B C3"
// ]);

define_pattern_resolver!(BroadcastLocalizedChat, [
    "48 89 74 24 10 57 48 83 EC 30 48 8B 01 41 8B F8 48 8B F2 ?? ?? ?? ?? ?? ?? 48 8B C8 48 8D"
]);

define_pattern_resolver![GetTBLGameMode, {
    EGS : ["40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51"], // EGS
    OTHER : ["40 53 48 83 EC 20 48 8B D9 48 85 C9 74 60 48 8B 01 FF 90 ?? ?? ?? ?? 48 85 C0 75 23 0F 1F 40 00 48 8B 5B 20 48 85 DB 74 11 48 8B 03 48 8B CB FF 90 ?? ?? ?? ?? 48 85 C0 74 E6 48 85 C0 74 2F 48 8B 98 28"]
}];




// Handle Game chat: commands, chat log, system messages
define_pattern_resolver!(ClientMessage, [
    "4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 73 18 49 8B D8 49 89 43 C8 48 8B F1 49 89 43 D0 49 89 43 D8 49 8D 43"
]);

mod client_message {
    use log::warn;
    use regex::Regex;

    use crate::chiv2::EChatType;
    use std::str::FromStr;

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
        let mut parts = line.splitn(2, ": ");
        warn!["parts: {:?}", parts];
        let msg_type_str = parts.next()?;
        let msg = parts.next()?;
        let msg_type = EChatType::from_str(msg_type_str).ok()?;
        Some((msg_type, msg))
    }
}

// void __thiscall APlayerController::ClientMessage(APlayerController *this,FString *S,FName Type,float MsgLifeTime)
CREATE_HOOK!(ClientMessage, (this:*mut c_void, S:*mut FString, Type:FName, MsgLifeTime: f32), {
    let cmd_store = Arc::clone(&LAST_COMMAND);
    // FIXME: Nihi: better way to access it?
    let string_ref: &FString = unsafe{ &*S };
    let message = string_ref.to_string();
    // TODO: Does this need to handle lines separately?
    // info!("[ClientMessages] S: \'{:?}\', Type: \'{}\'({}),  MsgLifeTime: \'{}\' ", message.replace("\r\n", " "), Type, Type.number, MsgLifeTime);
    info!("[client] \'{:?}\'", message.replace("\r\n", " "));

    // Chat commands
    // TODO: handle commands tranformed by ChatHooks
    //       e.g. ".hello" -> '<7>: Console Command: hello', Type: 'None'(0),  MsgLifeTime: '0'
    //       this needs to provide the playfabid somehow
    unsafe {        
        let cmd_filter = |c| ['/', '.'].contains(&c);
        match client_message::parse_chat_line(message.as_str()) {
            Some(chat) => {
                match chat.message.starts_with(cmd_filter) {
                    true => {
                        let cmd_trimmed = chat.message.trim_start_matches(cmd_filter);
                        debug!("-> Got console command from \'{}\' ch{}: {}", 
                            chat.name,
                            chat.channel,
                            cmd_trimmed);
                        // Set pending console command
                        // FIXME: Nihi: Add auth or allow only offline
                        *cmd_store.lock().unwrap() = Some(cmd_trimmed.trim().to_string());
                        debug!("Pending: {:?}", Some(cmd_trimmed.trim().to_string()));
                        // TODO: save command to log
                    }
                    false => {
                        debug!("-> User message: {:?}", chat);
                        // TODO: save user message
                    }
                };        
            }
            _ => {
                debug!("System message");
                if let Some(msg) = parse_chat_line(message.as_str()) {
                    warn!("Chat: channel {}, name {}, message {}, ", msg.channel, msg.name, msg.message);
                    if let Some(chat_mnsg) = client_message::parse_msg_line(msg.message) {
                        warn!("Text: {:?} {}", chat_mnsg.0, chat_mnsg.1);
                    }
                }
                // TODO: Write message to logfile
            }
        }
    }                  
});