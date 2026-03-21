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

#[derive(Clone, Debug, Default)]
pub struct BroadcastMessage {
    pub title: Option<String>,
    pub content: Option<String>,
    pub fields: Vec<(String, String)>,
    pub footer: Option<String>,
    pub color: Option<u32>,
    pub notify: Vec<Notify>,
}

impl BroadcastMessage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push((name.into(), value.into()));
        self
    }

    pub fn footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn color(mut self, color: u32) -> Self {
        self.color = Some(color);
        self
    }

    pub fn notify(mut self, notify: Notify) -> Self {
        self.notify.push(notify);
        self
    }

    pub fn content_ref(&self) -> Option<&str> {
        self.content.as_deref()
    }

    pub fn get_content(&self) -> Option<&str> {
        self.content_ref()
    }

    pub fn notify_roles(&self) -> &[Notify] {
        &self.notify
    }
}

impl From<BroadcastMessage> for CreateMessage {
    fn from(broadcast_message: BroadcastMessage) -> Self {
        let mut message = CreateMessage::new();
        let BroadcastMessage {
            title,
            content,
            fields,
            footer,
            color,
            notify: _,
        } = broadcast_message;

        let should_create_embed = title.is_some() || !fields.is_empty() || footer.is_some() || color.is_some();

        if should_create_embed {
            let mut embed = CreateEmbed::new();
            if let Some(title) = title {
                embed = embed.title(title);
            }
            if let Some(content) = content {
                embed = embed.description(content);
            }
            for (name, value) in fields {
                embed = embed.field(name, value, false);
            }
            if let Some(footer) = footer {
                embed = embed.footer(CreateEmbedFooter::new(footer));
            }
            if let Some(color) = color {
                embed = embed.color(color);
            }
            message = message.embed(embed);
        } else if let Some(content) = content {
            message = message.content(content);
        }

        message
    }
}

impl From<BroadcastMessage> for String {
    fn from(broadcast_message: BroadcastMessage) -> Self {
        let mut message = String::new();
        let BroadcastMessage {
            title,
            content,
            fields,
            footer,
            color,
            notify: _,
        } = broadcast_message;
        let is_rich = title.is_some() || !fields.is_empty() || footer.is_some() || color.is_some();

        if !is_rich {
            if let Some(content) = content {
                message.push_str(&content);
            }
        } else {
            if let Some(title) = title {
                message.push_str(&format!("{}\n", title));
            }

            if let Some(content) = content {
                message.push_str(&content);
            }

            for (name, value) in fields {
                message.push_str(&format!("\n{}: {}", name, value));
            }

            if let Some(footer) = footer {
                message.push_str(&format!("\n{}", footer));
            }
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
        Self::new().content(content)
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcast_message_conversions() {
        let msg = BroadcastMessage::new()
            .title("Test Title".to_string())
            .content("Test Content".to_string());
        
        let s: String = msg.clone().into();
        assert!(s.contains("Test Title"));
        assert!(s.contains("Test Content"));
        
        let _cm: CreateMessage = msg.into();
    }

    #[test]
    fn test_from_conversions() {
        let msg1 = BroadcastMessage::from("hello");
        assert_eq!(msg1.content_ref(), Some("hello"));
        
        let msg2 = BroadcastMessage::from("world".to_string());
        assert_eq!(msg2.content_ref(), Some("world"));
    }
}
