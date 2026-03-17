use std::sync::Arc;
use serenity::all::{ChannelId, Context, GatewayIntents, Http, Message, RoleId};
use serenity::Client;
use serenity::client::EventHandler as DiscordHandler;
use crate::events::bus::EventPublisher;
use crate::events::models::{ActorIdentity, ActorPermissions, ChatSource, ChatType, CommandActor, CommandSource, GameChatMessage, GameCommand, GameEvent, PermissionFlags};
use crate::events::integrations::broadcast_message::discord::DiscordBroadcastSubscriber;
use crate::events::integrations::game_event::chat::DiscordChatSink;
use crate::features::events::EVENT_SYSTEM;
use crate::features::tokio_runtime::TOKIO_RUNTIME;
use crate::{sinfo, swarn};

#[derive(Clone)]
pub struct DiscordConfig {
    pub bot_token: String,
    pub admin_role_id: Option<u64>,
    pub general_channel_id: ChannelId,
    pub admin_channel_id: Option<ChannelId>,
}

pub struct Discord {
    config: DiscordConfig,
    event_publisher: EventPublisher<GameEvent>,
}

pub fn initialize_discord_system(config: DiscordConfig) {
    let event_publisher = EVENT_SYSTEM.game_event_publisher.clone();

    TOKIO_RUNTIME.spawn(async move {
        let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
        
        let mut client = match Client::builder(&config.bot_token, intents)
            .event_handler(Discord { 
                config: config.clone(),
                event_publisher
            })
            .await {
            Ok(c) => c,
            Err(e) => {
                swarn!(f; "Failed to create Discord client: {}", e);
                return;
            }
        };

        register_discord_subscribers(config.clone(), client.http.clone()).await;
        add_discord_chat_sink(config.clone(), client.http.clone());

        if let Err(e) = client.start().await {
            swarn!(f; "Discord bot error: {}", e);
        }
    });
}

async fn register_discord_subscribers(config: DiscordConfig, http: Arc<Http>) {
    let broadcast_message_bus = &EVENT_SYSTEM.message_broadcast_event_bus;

    let discord_subscriber = DiscordBroadcastSubscriber::new(
        config.general_channel_id,
        config.admin_channel_id,
        config.admin_role_id.map(serenity::all::RoleId::new),
        http,
    );
    let _ = broadcast_message_bus.subscribe(Box::new(discord_subscriber)).await;
}

fn add_discord_chat_sink(discord_config: DiscordConfig, discord_http: Arc<Http>) {
    TOKIO_RUNTIME.spawn(async move {
        if let Some(admin_id) = discord_config.admin_channel_id {
            EVENT_SYSTEM.chat_relay_subscriber.add_sink(Box::new(DiscordChatSink::new(
                discord_config.general_channel_id,
                admin_id,
                discord_http,
            ))).await;
        } else {
            swarn!(f; "Discord chat sink not added because admin channel id is missing.");
        }
    });
}

#[async_trait::async_trait]
impl DiscordHandler for Discord {
    async fn ready(&self, _: Context, ready: serenity::model::gateway::Ready) {
        sinfo!(f; "Discord bot user '{}' is connected!", ready.user.name);
    }

    async fn message(&self, _ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        if Some(msg.channel_id) != Some(self.config.general_channel_id) && Some(msg.channel_id) != self.config.admin_channel_id {
            return;
        }

        let roles = msg.member.as_ref().map(|m| m.roles.clone()).unwrap_or_default();
        let is_admin = self.config.admin_role_id.map(|id| roles.contains(&RoleId::new(id))).unwrap_or(false);

        let actor = CommandActor {
            display_name: msg.author.name.clone(),
            identity: ActorIdentity::DiscordUser {
                user_id: msg.author.id,
                display_name: msg.author.name.clone(),
            },
            permissions: ActorPermissions {
                flags: if is_admin { PermissionFlags::ADMIN | PermissionFlags::MODERATOR | PermissionFlags::USER } else { PermissionFlags::USER },
            },
        };

        if msg.content.starts_with('!') {
            let content = &msg.content[1..];
            let parts = shlex::split(content).unwrap_or_default();
            if parts.is_empty() {
                return;
            }

            let name = parts[0].clone();
            let args = parts[1..].to_vec();
            let raw_args = if content.len() > name.len() {
                content[name.len()..].trim().to_string()
            } else {
                String::new()
            };

            self.event_publisher.publish(GameEvent::GameCommandEvent(GameCommand {
                name,
                args,
                raw_args,
                actor,
                source: CommandSource::Discord,
            }));
        } else if Some(msg.channel_id) == Some(self.config.general_channel_id) {
            self.event_publisher.publish(GameEvent::GameChatMessageEvent(GameChatMessage {
                chat_source: ChatSource::Discord,
                chat_type: ChatType::Global,
                sender: msg.author.name.clone(),
                message: msg.content.clone(),
            }));
        }
    }
}