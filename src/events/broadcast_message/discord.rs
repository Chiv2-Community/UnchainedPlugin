use crate::events::broadcast::{BroadcastMessage, Notify};
use crate::events::bus::Subscriber;
use crate::features::discord::{outbound::DiscordOutbox, SharedDiscordConfig};
use async_trait::async_trait;
use serenity::all::RoleId;
use serenity::builder::{CreateAllowedMentions, CreateMessage};

pub struct DiscordBroadcastSubscriber {
    discord_config: SharedDiscordConfig,
    outbox: DiscordOutbox,
}

impl DiscordBroadcastSubscriber {
    pub fn new(outbox: DiscordOutbox, discord_config: SharedDiscordConfig) -> Self {
        Self {
            discord_config,
            outbox,
        }
    }
}

#[async_trait]
impl Subscriber<BroadcastMessage> for DiscordBroadcastSubscriber {
    fn identifier(&self) -> &'static str {
        "DiscordBroadcastSubscriber"
    }

    async fn on_event(&mut self, event: &BroadcastMessage) {
        let config = self.discord_config;
        let admin_role_id = config.admin_role_id.map(RoleId::new);

        let mut discord_message: CreateMessage = event.clone().into();

        if config.mention_on_admin && !event.notify_roles().is_empty() {
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

        let allowed_mention = CreateAllowedMentions::new().roles(
            admin_role_id
                .filter(|_| config.mention_on_admin)
                .into_iter(),
        );
        discord_message = discord_message.allowed_mentions(allowed_mention);

        self.outbox
            .send(config.general_chat_channel_id, discord_message);
    }
}
