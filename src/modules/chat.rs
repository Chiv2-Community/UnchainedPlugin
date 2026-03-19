use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;
use futures::future::join_all;
use serenity::all::CreateMessage;
use serenity::http::Http;
use crate::sinfo;
use crate::events::bus::Subscriber;
use crate::events::models::{ChatSource, ChatType, GameChatMessage, GameEvent};
use crate::features::discord::{send_to_channel, SharedDiscordConfig};
use crate::game;
use crate::game::chivalry2::EChatType;

#[async_trait]
pub trait ChatSink: Send + Sync {
    async fn send(&self, chat_source: &ChatSource, chat_type: &ChatType, sender: &String, text: &String);
}

pub struct ConsoleChatSink;
#[async_trait]
impl ChatSink for ConsoleChatSink {
    async fn send(&self, chat_source: &ChatSource, chat_type: &ChatType, sender: &String, text: &String) {
        if chat_source == &ChatSource::Console { return };
        sinfo!("[{}] [{}] {}: {:?}", chat_source, chat_type, sender, text);
    }
}


pub struct GameChatSink;
#[async_trait]
impl ChatSink for GameChatSink {
    async fn send(&self, chat_source: &ChatSource, chat_type: &ChatType, sender: &String, text: &String) {
        if chat_source == &ChatSource::Game { return };

        let game_chat_type = match chat_type {
            ChatType::Admin => Some(EChatType::Admin),
            ChatType::Global => Some(EChatType::AllSay),
            ChatType::Team => Some(EChatType::TeamSay),
        };

        let message = format!("[{}] {}: {}", chat_source, sender, text);
        game::chivalry2::send_ingame_message(message, game_chat_type);
    }
}

pub struct DiscordChatSink {
    discord_config: SharedDiscordConfig,
    discord_http: Arc<Http>
}

impl DiscordChatSink {
    pub fn new(discord_config: SharedDiscordConfig, discord_http: Arc<Http>) -> Self {
        Self { discord_config, discord_http }
    }
}

#[async_trait]
impl ChatSink for DiscordChatSink {
    async fn send(&self, chat_source: &ChatSource, chat_type: &ChatType, sender: &String, text: &String) {
        if chat_source == &ChatSource::Discord { return };
        let config = self.discord_config.read().await.clone();

        let chat_type_string = match chat_type {
            ChatType::Admin => "[Admin] ",
            ChatType::Global => "",
            ChatType::Team => return,
        };

        let message = format!("**{}{}**: {}", chat_type_string, sender, text);
        send_to_channel(
            &self.discord_http,
            config.general_chat_channel_id,
            CreateMessage::new().content(message),
        )
        .await;
    }
}

#[derive(Clone)]
pub struct ChatRelaySubscriber {
    sinks: Arc<Mutex<Vec<Box<dyn ChatSink>>>>
}

impl ChatRelaySubscriber {
    pub fn new(sinks: Vec<Box<dyn ChatSink>>) -> Self {
        Self { sinks: Arc::new(Mutex::new(sinks)) }
    }

    pub async fn add_sink(&self, sink: Box<dyn ChatSink>) {
        self.sinks.lock().await.push(sink);
    }
}

#[async_trait]
impl Subscriber<GameEvent> for ChatRelaySubscriber {
    fn identifier(&self) -> &'static str {
        "ChatRelaySubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        match event {
            GameEvent::GameChatMessageEvent(GameChatMessage { chat_source, chat_type, sender, message, .. }) => {
                let sinks = self.sinks.lock().await;
                join_all(
                    sinks
                        .iter()
                        .map(|sink| sink.send(chat_source, chat_type, sender, message))
                ).await;
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::models::{ChatSource, ChatType, GameChatMessage, GameEvent};

    struct MockSink {
        messages: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl ChatSink for MockSink {
        async fn send(&self, _source: &ChatSource, _type: &ChatType, sender: &String, text: &String) {
            self.messages.lock().await.push(format!("{}: {}", sender, text));
        }
    }

    #[tokio::test]
    async fn test_chat_relay_subscriber_internal_mutability() {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let sink = Box::new(MockSink { messages: messages.clone() });
        
        let mut relay = ChatRelaySubscriber::new(vec![]);
        let relay_clone = relay.clone();
        
        // Add sink to one clone
        relay_clone.add_sink(sink).await;
        
        // Send event via the other clone
        let event = GameEvent::GameChatMessageEvent(GameChatMessage {
            chat_source: ChatSource::Game,
            chat_type: ChatType::Global,
            sender: "TestUser".to_string(),
            message: "Hello world".to_string(),
        });
        
        relay.on_event(&event).await;
        
        let msgs = messages.lock().await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0], "TestUser: Hello world");
    }
}

