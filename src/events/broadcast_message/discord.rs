use std::sync::Arc;
use async_trait::async_trait;
use serenity::all::{Http, RoleId};
use serenity::builder::{CreateAllowedMentions, CreateMessage};
use crate::events::broadcast::{BroadcastMessage, Notify};
use crate::events::bus::Subscriber;
use crate::features::discord::{send_to_channel, SharedDiscordConfig};

pub struct DiscordBroadcastSubscriber {
    discord_config: SharedDiscordConfig,
    discord_http: Arc<Http>,
}

impl DiscordBroadcastSubscriber {
    pub fn new(discord_http: Arc<Http>, discord_config: SharedDiscordConfig) -> Self {
        Self {
            discord_config,
            discord_http,
        }
    }
}

#[async_trait]
impl Subscriber<BroadcastMessage> for DiscordBroadcastSubscriber {
    fn identifier(&self) -> &'static str {
        "DiscordBroadcastSubscriber"
    }

    async fn on_event(&mut self, event: &BroadcastMessage) {
        let config = self.discord_config.read().await.clone();
        let admin_role_id = config.admin_role_id.map(RoleId::new);

        let mut discord_message: CreateMessage = event.clone().into();

        if !event.notify_roles().is_empty() {
            let mut content = event.content_ref().map(str::to_owned).unwrap_or_default();

            for notify_role in event.notify_roles().iter() {
                let notify_role_id = match notify_role {
                    Notify::Admin => admin_role_id,
                };

                if let Some(notify_role_id) = notify_role_id {
                    let notify_string = format!("\n<@&{}> ", notify_role_id);
                    content.push_str(&notify_string);
                }
            }

            discord_message = discord_message.content(content);
        }

        let allowed_mention = CreateAllowedMentions::new().roles(admin_role_id.into_iter());
        discord_message = discord_message.allowed_mentions(allowed_mention);

        send_to_channel(&self.discord_http, config.general_chat_channel_id, discord_message).await;
    }
}
