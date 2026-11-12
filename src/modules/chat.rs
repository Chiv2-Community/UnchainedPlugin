use crate::events::bus::Subscriber;
use crate::events::models::{ChatSource, ChatType, GameChatMessage, GameEvent, NativeChat};
use crate::features::discord::{outbound::DiscordOutbox, SharedDiscordConfig};
use crate::game;
use crate::game::chivalry2::EChatType;
use async_trait::async_trait;
use futures::future::join_all;
use serenity::all::CreateMessage;
use std::sync::Arc;
use tokio::sync::Mutex;

#[async_trait]
pub trait ChatSink: Send + Sync {
    async fn send(&self, message: &GameChatMessage);
}

pub struct LoggingChatSink;
#[async_trait]
impl ChatSink for LoggingChatSink {
    async fn send(&self, chat: &GameChatMessage) {
        if matches!(chat.chat_source, ChatSource::Console(_)) {
            return;
        };
        let (kind, channel) = match chat.chat_source {
            ChatSource::Native(NativeChat::Player { channel, .. }) => ("player", channel.as_str()),
            ChatSource::Native(NativeChat::System { channel }) => ("system", channel.as_str()),
            ChatSource::Discord(channel) | ChatSource::Console(channel) => ("external", channel.into()),
        };
        let source = chat.chat_source.to_string();
        let target = match chat.chat_source {
            ChatSource::Native(NativeChat::Player { channel: EChatType::AllSay, .. }) => "chat_log::all",
            ChatSource::Native(NativeChat::Player { channel: EChatType::TeamSay, .. }) => "chat_log::team",
            ChatSource::Native(NativeChat::System { channel: EChatType::AllSay }) if !chat.sender.is_empty() => "chat_log::all",
            ChatSource::Native(NativeChat::System { .. }) if chat.sender.is_empty() => "chat_log::system",
            ChatSource::Native(NativeChat::System { .. }) => "chat_log::system::sender",
            ChatSource::Discord(_) | ChatSource::Console(_) => "chat_log::external",
            _ => "chat_log",
        };
        let label = chat.chat_label();
        sinfo!(target: target, chat = channel, faction = chat.faction(), label = label.as_str(),
            chat_kind = kind, source = source.as_str(), sender = chat.sender.as_str();
            "{}", chat.message);
    }
}

pub struct GameChatSink;
#[async_trait]
impl ChatSink for GameChatSink {
    async fn send(&self, chat: &GameChatMessage) {
        if matches!(chat.chat_source, ChatSource::Native(_)) {
            return;
        };

        let game_chat_type = match chat.chat_type() {
            ChatType::Admin => Some(EChatType::Admin),
            ChatType::Global => Some(EChatType::AllSay),
            ChatType::Team => Some(EChatType::TeamSay),
        };

        let message = format!("[{}] {}: {}", chat.chat_source, chat.sender, chat.message);
        game::chivalry2::send_ingame_message(message, game_chat_type);
    }
}

pub struct DiscordChatSink {
    discord_config: SharedDiscordConfig,
    outbox: DiscordOutbox,
}

impl DiscordChatSink {
    pub fn new(discord_config: SharedDiscordConfig, outbox: DiscordOutbox) -> Self {
        Self {
            discord_config,
            outbox,
        }
    }
}

#[async_trait]
impl ChatSink for DiscordChatSink {
    async fn send(&self, chat: &GameChatMessage) {
        if matches!(chat.chat_source, ChatSource::Discord(_)) || !chat.is_relayable() {
            return;
        };
        let config = self.discord_config;

        let chat_type_string = match chat.chat_type() {
            ChatType::Admin => "[Admin] ",
            ChatType::Global => "",
            ChatType::Team => return,
        };

        let message = format!("**{}{}**: {}", chat_type_string, chat.sender, chat.message);
        self.outbox.send(
            config.general_chat_channel_id,
            CreateMessage::new().content(message),
        );
    }
}

#[derive(Clone)]
pub struct ChatRelaySubscriber {
    sinks: Arc<Mutex<Vec<Box<dyn ChatSink>>>>,
}

impl ChatRelaySubscriber {
    pub fn new(sinks: Vec<Box<dyn ChatSink>>) -> Self {
        Self {
            sinks: Arc::new(Mutex::new(sinks)),
        }
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
        if let GameEvent::GameChatMessageEvent(message) = event {
            let sinks = self.sinks.lock().await;
            join_all(sinks.iter().map(|sink| sink.send(message))).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::models::{ChatSource, ChatType, GameChatMessage, GameEvent};

    #[tokio::test]
    async fn logging_sink_preserves_raw_chat_and_source_metadata() {
        use crate::game::chivalry2::EFaction;
        use log4rs::encode::{Encode, writer::simple::SimpleWriter};
        use std::sync::Mutex as StdMutex;
        struct Capture(StdMutex<Vec<serde_json::Value>>);
        static CAPTURE: Capture = Capture(StdMutex::new(Vec::new()));
        impl log::Log for Capture {
            fn enabled(&self, _: &log::Metadata) -> bool { true }
            fn log(&self, record: &log::Record) {
                let text = record.args().to_string();
                if text != "CHAT_TEST\r\nsecond line" { return; }
                let mut bytes = Vec::new();
                log4rs::encode::json::JsonEncoder::new().encode(&mut SimpleWriter(&mut bytes), record).unwrap();
                self.0.lock().unwrap().push(serde_json::from_slice(&bytes).unwrap());
            }
            fn flush(&self) {}
        }
        log::set_logger(&CAPTURE).unwrap();
        log::set_max_level(log::LevelFilter::Trace);
        for (source, sender, target, channel, faction, label) in [
            (ChatSource::Native(NativeChat::Player { channel: EChatType::TeamSay, faction: Some(EFaction::Mason) }), "Player", "chat_log::team", "TeamSay", "Mason", "MASON"),
            (ChatSource::Native(NativeChat::Player { channel: EChatType::AllSay, faction: Some(EFaction::Mason) }), "Player", "chat_log::all", "AllSay", "Mason", "ALL"),
            (ChatSource::Native(NativeChat::Player { channel: EChatType::TeamSay, faction: Some(EFaction::None) }), "Player", "chat_log::team", "TeamSay", "None", "SPECTATOR"),
            (ChatSource::Native(NativeChat::System { channel: EChatType::ServerSay }), "", "chat_log::system", "ServerSay", "System", "SYSTEM"),
            (ChatSource::Native(NativeChat::System { channel: EChatType::ServerSay }), "Console", "chat_log::system::sender", "ServerSay", "System", "SYSTEM"),
            (ChatSource::Native(NativeChat::System { channel: EChatType::AllSay }), "Console", "chat_log::all", "AllSay", "System", "ALL"),
            (ChatSource::Discord(ChatType::Global), "Player", "chat_log::external", "Global", "Discord", "DISCORD"),
        ] {
            let chat = GameChatMessage {
                chat_source: source, sender: sender.into(), message: "CHAT_TEST\r\nsecond line".into(),
            };
            LoggingChatSink.send(&chat).await;
            let records = std::mem::take(&mut *CAPTURE.0.lock().unwrap());
            assert_eq!(records.len(), 1);
            let record = &records[0];
            assert_eq!(record["target"], target);
            assert_eq!(record["message"], chat.message);
            let fields = &record["attributes"];
            assert_eq!(fields["sender"], chat.sender);
            assert_eq!(fields["source"], source.to_string());
            assert_eq!(fields["chat"], channel);
            assert_eq!(fields["faction"], faction);
            assert_eq!(fields["label"], label);
            assert!(fields.get("inline_colors").is_none());
        }
    }

    struct MockSink {
        messages: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl ChatSink for MockSink {
        async fn send(&self, message: &GameChatMessage) {
            self.messages
                .lock()
                .await
                .push(format!("{}: {}", message.sender, message.message));
        }
    }

    #[tokio::test]
    async fn test_chat_relay_subscriber_internal_mutability() {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let sink = Box::new(MockSink {
            messages: messages.clone(),
        });

        let mut relay = ChatRelaySubscriber::new(vec![]);
        let relay_clone = relay.clone();

        // Add sink to one clone
        relay_clone.add_sink(sink).await;

        // Send event via the other clone
        let event = GameEvent::GameChatMessageEvent(GameChatMessage {
            chat_source: ChatSource::Native(NativeChat::Player { channel: EChatType::AllSay, faction: None }),
            sender: "TestUser".to_string(),
            message: "Hello world".to_string(),
        });

        relay.on_event(&event).await;

        let msgs = messages.lock().await;
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0], "TestUser: Hello world");
    }
}
