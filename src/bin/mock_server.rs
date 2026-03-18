use serenity::all::UserId;
use UnchainedPlugin::features::discord::DiscordConfig;
use UnchainedPlugin::events::models::{GameChatMessage, Join, Kill, GameEvent, ChatSource, ChatType, CommandActor, CommandSource, CommandExecuted};
use UnchainedPlugin::features::events::EVENT_SYSTEM;
use UnchainedPlugin::sinfo;
use UnchainedPlugin::tools::logger::init_syslog;
use std::io::{self, Write};
use UnchainedPlugin::features::discord::initialize_discord_system;

fn main() {
    println!("🚀 Discord Mock Server Starting!");
    
    let config = DiscordConfig {
        bot_token: "YOUR_TOKEN_HERE".to_string(),
        admin_role_id: None,
        general_channel_id: serenity::all::ChannelId::new(1),
        admin_channel_id: None,
        mention_on_admin: true,
    };

    initialize_discord_system(config.clone());

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
                
                EVENT_SYSTEM.game_event_publisher.publish(GameEvent::CommandExecutedEvent(CommandExecuted {
                    name,
                    args,
                    raw_args: msg,
                    actor: CommandActor::from_discord(
                        UserId::new(std::num::NonZeroU64::new(1234).unwrap().into()),
                        "MockDiscUser".into(),
                        &[],
                        config.clone()
                    ),
                    source: CommandSource::Discord,
                }));
            }
            ["exit"] => break,
            _ => println!("Unknown command. Try 'join Arthur'"),
        }
    }
}
