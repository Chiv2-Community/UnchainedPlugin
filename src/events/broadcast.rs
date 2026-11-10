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

pub use crate::events::rich_text::{RichText, RichTextChunk};

#[derive(Clone, Debug, Default)]
pub struct BroadcastMessage {
    pub title: Option<RichText>,
    pub content: Option<RichText>,
    pub fields: Vec<(RichText, RichText)>,
    pub footer: Option<RichText>,
    pub color: Option<u32>,
    pub notify: Vec<Notify>,
}

impl BroadcastMessage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<RichText>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn content(mut self, content: impl Into<RichText>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn field(mut self, name: impl Into<RichText>, value: impl Into<RichText>) -> Self {
        self.fields.push((name.into(), value.into()));
        self
    }

    pub fn footer(mut self, footer: impl Into<RichText>) -> Self {
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

    pub fn content_ref(&self) -> Option<&RichText> {
        self.content.as_ref()
    }

    pub fn get_content(&self) -> Option<&RichText> {
        self.content_ref()
    }

    pub fn notify_roles(&self) -> &[Notify] {
        &self.notify
    }

    pub fn to_plain_text(&self) -> String {
        self.render_text(RichText::to_plain_text)
    }

    pub fn to_game_chat(&self) -> String {
        self.render_text(RichText::to_game_chat)
    }

    fn render_text(&self, render: fn(&RichText) -> String) -> String {
        let mut message = String::new();
        if let Some(title) = &self.title {
            message.push_str(&render(title));
            message.push('\n');
        }
        if let Some(content) = &self.content {
            message.push_str(&render(content));
        }
        for (name, value) in &self.fields {
            message.push_str(&format!("\n{}: {}", render(name), render(value)));
        }
        if let Some(footer) = &self.footer {
            message.push_str(&format!("\n{}", render(footer)));
        }
        message
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
                embed = embed.title(title.to_discord());
            }
            if let Some(content) = content {
                embed = embed.description(content.to_discord());
            }
            for (name, value) in fields {
                embed = embed.field(name.to_discord(), value.to_discord(), false);
            }
            if let Some(footer) = footer {
                embed = embed.footer(CreateEmbedFooter::new(footer.to_discord()));
            }
            if let Some(color) = color {
                embed = embed.color(color);
            }
            message = message.embed(embed);
        } else if let Some(content) = content {
            message = message.content(content.to_discord());
        }

        message
    }
}

impl From<BroadcastMessage> for String {
    fn from(broadcast_message: BroadcastMessage) -> Self {
        broadcast_message.to_plain_text()
    }
}

impl From<String> for BroadcastMessage {
    fn from(content: String) -> Self {
        Self::new().content(content)
    }
}

impl From<RichText> for BroadcastMessage {
    fn from(content: RichText) -> Self {
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

    fn formatted_message() -> BroadcastMessage {
        BroadcastMessage::new()
            .title(crate::rich_text!("**title**"))
            .content(crate::rich_text!("**body**"))
            .field(crate::rich_text!("**name**"), crate::rich_text!("**value**"))
            .footer(crate::rich_text!("**footer**"))
    }

    #[test]
    fn plain_layout_preserves_field_order() {
        assert_eq!(formatted_message().to_plain_text(), "title\nbody\nname: value\nfooter");
    }

    #[test]
    fn game_content_uses_emphasis_tags() {
        let tag = "<RichText.Emphasis>bold</>";
        let message = BroadcastMessage::new().content(crate::rich_text!("**bold**"));
        assert_eq!(message.to_game_chat(), tag);
    }

    #[test]
    fn discord_embed_formats_each_field() {
        let discord = serde_json::to_value(CreateMessage::from(formatted_message())).unwrap();
        let embed = &discord["embeds"][0];
        assert_eq!(embed["title"], "**title**");
        assert_eq!(embed["description"], "**body**");
        assert_eq!(embed["fields"][0]["name"], "**name**");
        assert_eq!(embed["fields"][0]["value"], "**value**");
        assert_eq!(embed["footer"]["text"], "**footer**");
    }

}
