use std::sync::Arc;
use async_trait::async_trait;
use serenity::all::{ChannelId, Http, RoleId};
use serenity::builder::{CreateAllowedMentions, CreateMessage};
use crate::events::broadcast::{BroadcastMessage, Notify};
use crate::events::bus::Subscriber;

pub struct DiscordBroadcastSubscriber {
    channel_id: ChannelId,
    admin_channel_id: Option<ChannelId>,
    admin_role_id: Option<RoleId>,
    discord_http: Arc<Http>,
}

impl DiscordBroadcastSubscriber {
    pub fn new(
        channel_id: ChannelId,
        admin_channel_id: Option<ChannelId>,
        admin_role_id: Option<RoleId>,
        discord_http: Arc<Http>,
    ) -> Self {
        Self {
            channel_id,
            admin_channel_id,
            admin_role_id,
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
        let mut discord_message: CreateMessage = event.clone().into();
        let http = Arc::clone(&self.discord_http);
        let channel_id = self.channel_id;

        if !event.notify.is_empty() {
            let mut content = event.content.clone().unwrap_or_default();

            for notify_role in event.notify.iter() {
                let notify_role_id = match notify_role {
                    Notify::Admin => self.admin_role_id,
                    // Add others as we create them
                };

                if let Some(notify_role_id) = notify_role_id {
                    let notify_string = format!("\n<@&{}> ", notify_role_id);
                    content.push_str(&notify_string);
                }
            }

            discord_message = discord_message.content(content);
        }

        let allowed_mention =
            CreateAllowedMentions::new()
                .roles(self.admin_role_id.into_iter());



        tokio::spawn(async move {
            let _ = channel_id.send_message(
                &http,
                discord_message
                    .allowed_mentions(allowed_mention)
            ).await;
        });
    }
}
