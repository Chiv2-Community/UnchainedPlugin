use serenity::{all::CreateMessage, async_trait, builder::CreateEmbed, model::prelude::*, prelude::*};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{game::chivalry2::EChatType, resolvers::unchained_integration::CHAT_QUEUE, serror, tools::hook_globals::globals};

#[derive(Clone)]
pub struct ChatMessage {
    pub msg: String,
    pub chat_type: EChatType
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub bot_token: String,
    pub channel_id: u64,
}

/// Messages moving from Discord -> Game
pub struct IncomingChatMessage {
    pub user: String,
    pub text: String,
}

/// Events moving from Game -> Discord
pub enum OutgoingEvent {
    Chat { user: String, text: String },
    PlayerJoin { name: String },
    MatchWon { winner_name: String, map: String },
}

struct Handler {
    config: DiscordConfig,
    to_game: Sender<IncomingChatMessage>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        serror!("{} is connected!", ready.user.name);
        
        // Check if we actually have the Message Content intent
        // If this prints 'false', Discord is stripping your message data
        serror!("Has Message Content Intent: {}", 
            ready.application.flags.contains(serenity::model::application::ApplicationFlags::GATEWAY_MESSAGE_CONTENT)
        );
    }
    async fn message(&self, ctx: Context, msg: Message) {
        // 1. Ignore if the message was sent by a bot (including this bot)
        if msg.author.bot {
            return;
        }

        // 2. Ignore if it's not the channel we are looking for
        if msg.channel_id.get() != self.config.channel_id {
            return;
        }

        // 3. Process the message...
        let _ = self.to_game.send(IncomingChatMessage {
            user: msg.author.name.clone(),
            text: msg.content.clone(),
        });
    }
}

#[derive(Debug)]
pub struct DiscordBridge {
    pub incoming: Receiver<IncomingChatMessage>,
    outgoing: Sender<OutgoingEvent>,
}
pub fn start_discord_listener() {
    std::thread::spawn(move || {
        println!("Discord listener thread started...");
        loop {
            if let Some(bridge) = globals().DISCORD_BRIDGE.get() {
                match bridge.incoming.try_recv() {
                    Ok(msg) => {
                        println!("Thread received from bridge: {}", msg.text);
                        if let Ok(mut queue) = CHAT_QUEUE.lock() {
                            queue.push(ChatMessage { msg: format!("<D>{}: {}", msg.user, msg.text), chat_type: EChatType::AllSay });
                        }
                    },
                    Err(_) => { /* No messages, keep waiting */ }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
}

impl DiscordBridge {
    pub fn recv_game_message(&self, _msg_type: EChatType, player_name: &str, message: &str) {
        // We just "fire and forget" the event into our channel
        crate::sinfo!(f; "Sending message to discord");
        let event = OutgoingEvent::Chat {
            user: player_name.to_string(),
            text: message.to_string(),
        };

        if let Err(e) = self.outgoing.send(event) {
            eprintln!("Failed to queue discord message: {}", e);
        }
    }

    pub fn new(config: DiscordConfig) -> Self {
        let (in_tx, in_rx) = unbounded();
        let (out_tx, out_rx) = unbounded();
        
        let cfg_clone = config.clone();

        // Spawn the Discord Thread
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async move {
                // let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
                let intents = serenity::model::gateway::GatewayIntents::non_privileged() 
                        | serenity::model::gateway::GatewayIntents::MESSAGE_CONTENT;
                let mut client = Client::builder(&cfg_clone.bot_token, intents)
                    .event_handler(Handler { config: cfg_clone.clone(), to_game: in_tx })
                    .await
                    .expect("Err creating client");

                // FIX: In 0.12, use client.http directly
                let http = client.http.clone();
                
                // let builder = CreateMessage::new()
                // .content("A match has started!")
                // .enforce_nonce(true); // Optional: prevents duplicate sends
                // let channel = ChannelId::new(config.channel_id);
                // if let Err(why) = channel.send_message(&http, builder).await {
                //     println!("Error sending message: {:?}", why);
                // }

                tokio::spawn(async move {
                    while let Ok(event) = out_rx.recv() {
                        // FIX: Use ChannelId::new()
                        let channel = ChannelId::new(cfg_clone.channel_id);
                        
                        match event {
                            OutgoingEvent::Chat { user, text } => {
                                let content = format!("**{}**: {}", user, text);
                                let _ = channel.say(&http, content).await;
                            }
                            OutgoingEvent::PlayerJoin { name } => {
                                // FIX: 0.12 uses a new builder pattern for messages
                                let embed = Self::build_join_embed(name);
                                let builder = CreateMessage::new().add_embed(embed);
                                let _ = channel.send_message(&http, builder).await;
                            }
                            OutgoingEvent::MatchWon { winner_name, map } => {
                                let embed = Self::build_win_embed(winner_name, map);
                                let builder = CreateMessage::new().add_embed(embed);
                                let _ = channel.send_message(&http, builder).await;
                            }
                        };
                    }
                });

                client.start().await.expect("Discord bot crashed");
            });
        });

        Self::start_internal_listener();

        Self { incoming: in_rx, outgoing: out_tx }
    }
    
    fn start_internal_listener() {
        std::thread::spawn(move || {
            println!("Discord listener thread started...");
            loop {
                // We access the global bridge to pull messages
                if let Some(bridge) = globals().DISCORD_BRIDGE.get() {
                    // println!("got bridge");
                    while let Ok(msg) = bridge.incoming.try_recv() {
                        if let Ok(mut queue) = CHAT_QUEUE.lock() {
                            queue.push(ChatMessage { msg: format!("<D>{}: {}", msg.user, msg.text), chat_type: EChatType::AllSay });
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
    }

    pub fn send_event(&self, event: OutgoingEvent) {
        let _ = self.outgoing.send(event);
    }

    fn build_join_embed(name: String) -> CreateEmbed {
        // No 'let mut' and no '&' borrow. Just chain and return.
        CreateEmbed::new()
            .title("📥 Player Joined")
            .description(format!("**{}** has entered the fray!", name))
            .color(0x2ecc71) // Green
    }

    fn build_win_embed(winner: String, map: String) -> CreateEmbed {
        let server_name = globals().cli_args
            .find_ini_value(&[("Game", "[/Script/TBL.TBLGameMode]", "ServerName")])
            .unwrap_or("Server");
        CreateEmbed::new()
            .title(format!("🏆 Victory ({})", server_name))
            .field("Winner", winner, true)
            .field("Map", map, true)
            .color(0xf1c40f) // Gold
    }
}