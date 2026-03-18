//! The broadcast system is used for distributing rich messages to multiple message backends.
//!
//! # Architecture
//!
//! [`BroadcastMessage`] is the core model that represents a notification to be sent to users.
//! It supports both simple text content and "rich" features (title, fields, footer, color, embed support)
//! that can be rendered by sophisticated backends like Discord.
//!
//! # Backends
//!
//! The system broadcasts messages to several backends via subscribers:
//! - **Discord**: Messages are converted to embeds or content and sent via [`DiscordBroadcastSubscriber`].
//! - **Game Chat**: Messages are sent to the in-game chat via [`InGameChatBroadcastSubscriber`].
//! - **Server Console**: Messages are printed to the server console via [`ConsoleChatBroadcastSubscriber`].
//!
//! # Usage in Project
//!
//! Broadcasts are triggered throughout the project for:
//! - **Game Events**: Announcements like killstreaks, player joins/leaves.
//! - **Admin Notifications**: Critical server alerts or crash reports (using [`Notify::Admin`]).
//! - **Commands**: Feedback from commands like `!say` or `!vote`.
//!
//! Messages are typically published to the `message_broadcast_event_bus` defined in `src/features/events.rs`.
use serenity::builder::{CreateEmbed, CreateEmbedFooter, CreateMessage};

#[derive(Clone, Debug)]
pub enum Notify {
    Admin
}

#[derive(Clone, Debug)]
pub struct BroadcastMessage {
    pub title: Option<String>,
    pub content: Option<String>,
    pub fields: Option<Vec<(String, String)>>,
    pub footer: Option<String>,
    pub color: Option<u32>,
    pub notify: Vec<Notify>
}

impl BroadcastMessage {
    pub fn new() -> Self {
        Self {
            title: None,
            content: None,
            fields: None,
            footer: None,
            color: None,
            notify: Vec::new(),
        }
    }

    fn is_rich(&self) -> bool {
        self.title.is_some() || self.fields.is_some() || self.footer.is_some() || self.color.is_some()
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn content(mut self, content: String) -> Self {
        self.content = Some(content);
        self
    }

    pub fn field(mut self, name: String, value: String) -> Self {
        let fields = self.fields.get_or_insert_with(Vec::new);
        fields.push((name, value));
        self
    }

    pub fn footer(mut self, footer: String) -> Self {
        self.footer = Some(footer);
        self
    }

    pub fn color(mut self, color: u32) -> Self {
        self.color = Some(color);
        self
    }

    pub fn get_content(&self) -> Option<&String> {
        self.content.as_ref()
    }
}

impl Into<CreateMessage> for BroadcastMessage {
    fn into(self) -> CreateMessage {
        let mut message = CreateMessage::new();

        let should_create_embed = self.is_rich();

        if should_create_embed {
            let mut embed = CreateEmbed::new();
            if let Some(title) = self.title {
                embed = embed.title(title);
            }
            if let Some(content) = self.content {
                embed = embed.description(content);
            }
            if let Some(fields) = self.fields {
                for (name, value) in fields {
                    embed = embed.field(name, value, false);
                }
            }
            if let Some(footer) = self.footer {
                embed = embed.footer(CreateEmbedFooter::new(footer));
            }
            if let Some(color) = self.color {
                embed = embed.color(color);
            }
            message = message.embed(embed);
        } else if let Some(content) = self.content {
            message = message.content(content);
        }

        message
    }
}

impl Into<String> for BroadcastMessage {
    fn into(self) -> String {
        let mut message = String::new();

        if !self.is_rich() && self.content.is_some() {
            message.push_str(&self.content.unwrap());
        } else if self.is_rich() {
            message.push_str("\n\n=====================\n\n");

            if let Some(title) = self.title {
                message.push_str(&format!("------------ {} ------------\n\n", title));
            }

            if let Some(content) = self.content {
                message.push_str(&content);
            }

            if let Some(fields) = self.fields {
                for (name, value) in fields {
                    message.push_str(&format!("\n**{}**: {}", name, value));
                }
            }

            if let Some(footer) = self.footer {
                message.push_str(&format!("\n\n------------ {} ------------", footer));
            }

            message.push_str("\n\n=====================\n\n");
        }


        message
    }
}

impl From<String> for BroadcastMessage {
    fn from(content: String) -> Self {
        Self::new().content(content)
    }
}

impl From<&str> for BroadcastMessage {
    fn from(content: &str) -> Self {
        Self::new().content(content.to_string())
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use serenity::all::Http;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_broadcast_message_conversions() {
        let msg = BroadcastMessage::new()
            .title("Test Title".to_string())
            .content("Test Content".to_string());
        
        let s: String = msg.clone().into();
        assert!(s.contains("Test Title"));
        assert!(s.contains("Test Content"));
        
        let _cm: CreateMessage = msg.into();
    }

    #[tokio::test]
    async fn test_from_conversions() {
        let msg1 = BroadcastMessage::from("hello");
        assert_eq!(msg1.content, Some("hello".to_string()));
        
        let msg2 = BroadcastMessage::from("world".to_string());
        assert_eq!(msg2.content, Some("world".to_string()));
    }
}
