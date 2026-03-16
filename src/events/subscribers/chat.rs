use std::sync::Arc;
use async_trait::async_trait;
use futures::future::join_all;
use serenity::all::{ChannelId, CreateMessage};
use serenity::http::Http;
use strum::Display;
use crate::events::bus::Subscriber;
use crate::events::models::{ChatSource, ChatType, GameChatMessage, GameEvent};
use crate::game;
use crate::game::chivalry2::EChatType;

#[async_trait]
pub trait ChatSink: Send + Sync {
    async fn send(&self, chat_source: &ChatSource, chat_type: &ChatType, text: &String);
}

struct ConsoleChatSink;
#[async_trait]
impl ChatSink for ConsoleChatSink {
    async fn send(&self, chat_source: &ChatSource, chat_type: &ChatType, text: &String) {
        if chat_source == &ChatSource::Console { return };
        println!("[{}] [{}] {:?}", chat_source, chat_type, text);
    }
}


struct GameChatSink;
#[async_trait]
impl ChatSink for GameChatSink {
    async fn send(&self, chat_source: &ChatSource, chat_type: &ChatType, text: &String) {
        if chat_source == &ChatSource::Game { return };

        let game_chat_type = match chat_type {
            ChatType::Admin => Some(EChatType::Admin),
            ChatType::Global => Some(EChatType::AllSay),
            ChatType::Team => Some(EChatType::TeamSay),
        };

        let message = format!("[{}] {}", chat_source, text);
        game::chivalry2::send_ingame_message(message, game_chat_type);
    }
}

struct DiscordChatSink {
    general_chat_channel_id: ChannelId,
    admin_chat_channel_id: ChannelId,
    discord_http: Arc<Http>
}

impl DiscordChatSink {
    pub fn new(general_chat_channel_id: ChannelId, admin_chat_channel_id: ChannelId, discord_http: Arc<Http>) -> Self {
        Self { general_chat_channel_id, admin_chat_channel_id, discord_http }
    }
}

#[async_trait]
impl ChatSink for DiscordChatSink {
    async fn send(&self, chat_source: &ChatSource, chat_type: &ChatType, text: &String) {
        if chat_source == &ChatSource::Discord { return };

        let maybe_channel_id = match chat_type {
            ChatType::Admin => Some(self.admin_chat_channel_id),
            ChatType::Global => Some(self.general_chat_channel_id),
            ChatType::Team => None, // Don't want to send team messages to discord in case they're actually private/strategic
        };

        if let Some(channel_id) = maybe_channel_id {
            let message = format!("[{}] {}", chat_source, text);
            let _ = channel_id
                .send_message(&self.discord_http, CreateMessage::new().content(message))
                .await
                .inspect_err(|e| eprintln!("Failed to send Discord message: {:?}", e));
        };
    }
}

struct ChatRelaySubscriber {
    sinks: Vec<Box<dyn ChatSink>>
}

#[async_trait]
impl Subscriber<GameEvent> for ChatRelaySubscriber {
    fn identifier(&self) -> &'static str {
        "ChatRelaySubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        match event {
            GameEvent::GameChatMessageEvent(GameChatMessage { chat_source, chat_type, sender, message }) => {
                join_all(
                    self.sinks
                        .iter()
                        .map(|sink| sink.send(chat_source, chat_type, message))
                ).await;
            },
            _ => {}
        }
    }
}



