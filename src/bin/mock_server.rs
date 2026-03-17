use serenity::all::UserId;
use UnchainedPlugin::discord::config::DiscordConfig;
use UnchainedPlugin::events::models::{GameChatMessage, Join, Kill, GameEvent, ChatSource, ChatType, CommandActor, CommandSource, GameCommand};
use UnchainedPlugin::features::events::EVENT_SYSTEM;
use UnchainedPlugin::discord::{ConsoleChatSink, DISCORD_HANDLE, DiscordBridge, SleuthContext};
use UnchainedPlugin::{serror, sinfo};
use UnchainedPlugin::tools::logger::init_syslog;
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
            config: config.clone()
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
                EVENT_SYSTEM.game_event_publisher.publish(GameEvent::JoinEvent(Join { name: name.to_string() }));
            }
            ["kill", k, v] => {
                EVENT_SYSTEM.game_event_publisher.publish(GameEvent::KillEvent(Kill {
                    killer: k.to_string(), 
                    victim: v.to_string(), 
                    weapon: "MockSword".to_string() 
                }));
            }
            ["chat", ..] => {
                let msg = parts[1..].join(" ");
                EVENT_SYSTEM.game_event_publisher.publish(GameEvent::GameChatMessageEvent(GameChatMessage {
                    chat_source: ChatSource::Game,
                    chat_type: ChatType::Admin,
                    sender: "MockPlayer".to_string(), 
                    message: msg,
                }));
            }
            ["dchat", ..] => {
                let msg = parts[1..].join(" ");
                let parts: Vec<String> = msg.split_whitespace().map(|s| s.to_string()).collect();
                let name = parts.get(0).cloned().unwrap_or_default();
                let args = if parts.len() > 1 { parts[1..].to_vec() } else { vec![] };
                
                EVENT_SYSTEM.game_event_publisher.publish(GameEvent::GameCommandEvent(GameCommand {
                    name,
                    args,
                    raw_args: msg,
                    actor: CommandActor::from_discord(
                        UserId::new(1234),
                        "MockDiscUser".into(),
                        &[],
                        &config
                    ),
                    source: CommandSource::Discord,
                }));
            }
            ["exit"] => break,
            _ => println!("Unknown command. Try 'join Arthur'"),
        }
    }
}