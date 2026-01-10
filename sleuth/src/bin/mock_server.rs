use serenity::all::UserId;
use sleuthlib::discord::config::DiscordConfig;
use sleuthlib::discord::notifications::{CommandRequest, GameChatMessage, JoinEvent, KillEvent};
use sleuthlib::discord::{ConsoleChatSink, DISCORD_HANDLE, DiscordBridge, SleuthContext};
use sleuthlib::game::chivalry2::EChatType;
use sleuthlib::{dispatch, serror, sinfo};
use sleuthlib::tools::logger::init_syslog;
use std::io::{self, Write};
use std::sync::Arc;


fn main() {
    println!("🚀 Discord Mock Server Starting!");
    // 1. Load your real config so the mock bot actually connects to Discord
    // sinfo!(f; "Starting discord bridge");
    
    let config_path = "discord_config_mock.json";
    let config = DiscordConfig::load(config_path, true).unwrap_or_else(|e| {
        serror!("Configuration Error, loading default: {}", e);
        DiscordConfig::default()
    });
    let ctx = Arc::new(SleuthContext {
            chat: Arc::new(ConsoleChatSink),
            config: config
        });
    let handle = DiscordBridge::init(config_path, ctx);
    DISCORD_HANDLE.set(handle)
        .expect("Discord Handle was already initialized!");

    init_syslog().expect("Failed to init syslog");
    sinfo!("Discord Mock Server Started!");
    sinfo!("Commands: join <name>, kill <killer> <victim>, chat <msg>, exit");

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let parts: Vec<&str> = input.trim().split_whitespace().collect();

        match parts.as_slice() {
            ["join", name] => {
                dispatch!(JoinEvent { name: name.to_string() });
            }
            ["kill", k, v] => {
                dispatch!(KillEvent { 
                    killer: k.to_string(), 
                    victim: v.to_string(), 
                    weapon: "MockSword".to_string() 
                });
            }
            ["chat", ..] => {
                let msg = parts[1..].join(" ");
                dispatch!(GameChatMessage { 
                    sender: "MockPlayer".to_string(), 
                    message: msg,
                    chat_type: EChatType::Admin, 
                });
            }
            ["dchat", ..] => {
                let msg = parts[1..].join(" ");
                dispatch!(CommandRequest {
                    command: msg,
                    user: "MockDiscUser".into(),
                    user_id: UserId::new(1234),
                    user_roles: [].into(),
                });
            }
            ["exit"] => break,
            _ => println!("Unknown command. Try 'join Arthur'"),
        }
    }
}