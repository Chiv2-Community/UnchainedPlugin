use std::sync::Arc;
use serenity::all::{ChannelId, Context, CreateMessage, GatewayIntents, Http, Message, RoleId};
use serenity::Client;
use serenity::client::EventHandler as DiscordHandler;
use tokio::sync::RwLock;
use crate::events::bus::EventPublisher;
use crate::events::models::{ActorIdentity, ActorPermissions, ChatSource, ChatType, CommandActor, CommandSource, GameChatMessage, CommandRequest, GameEvent, PermissionFlags};
use crate::events::broadcast_message::discord::DiscordBroadcastSubscriber;
use crate::modules::chat::DiscordChatSink;
use crate::modules::discord::admin_alert::AdminAlertModule;
use crate::modules::discord::dashboard::DashboardSubscriber;
use crate::modules::discord::event_log::EventLogSubscriber;
use crate::features::events::EVENT_SYSTEM;
use crate::features::tokio_runtime::TOKIO_RUNTIME;
use crate::{sinfo, swarn};

#[derive(Clone)]
pub struct DiscordConfig {
    pub bot_token: String,
    pub admin_role_id: Option<u64>,
    pub dashboard_channel_id: Option<ChannelId>,
    pub general_chat_channel_id: Option<ChannelId>,
    pub admin_notification_channel_id: Option<ChannelId>,
    pub event_log_channel_id: Option<ChannelId>,
    pub mention_on_admin: bool,
}

pub type SharedDiscordConfig = Arc<RwLock<DiscordConfig>>;

pub async fn send_to_channel(
    http: &Http,
    channel_id: Option<ChannelId>,
    message: CreateMessage,
) {
    match channel_id {
        Some(id) => {
            if let Err(e) = id.send_message(http, message).await {
                swarn!(f; "Failed to send to channel {}: {}", id, e);
            }
        }
        None => {
            swarn!(f; "Attempted to send to a channel that is not configured.");
        }
    }
}

pub struct Discord {
    config: SharedDiscordConfig,
    event_publisher: EventPublisher<GameEvent>,
}

pub fn initialize_discord_system(config: DiscordConfig) -> SharedDiscordConfig {
    let event_publisher = EVENT_SYSTEM.game_event_publisher.clone();
    let shared_config = Arc::new(RwLock::new(config));
    let task_config = Arc::clone(&shared_config);

    TOKIO_RUNTIME.spawn(async move {
        let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

        let bot_token = {
            let config = task_config.read().await;
            config.bot_token.clone()
        };

        let mut client = match Client::builder(&bot_token, intents)
            .event_handler(Discord { 
                config: Arc::clone(&task_config),
                event_publisher
            })
            .await {
            Ok(c) => c,
            Err(e) => {
                swarn!(f; "Failed to create Discord client: {}", e);
                return;
            }
        };

        register_discord_subscribers(Arc::clone(&task_config), client.http.clone()).await;
        
        let should_add_chat_sink = {
            let config = task_config.read().await;
            config.general_chat_channel_id.is_some()
        };

        if should_add_chat_sink {
            add_discord_chat_sink(Arc::clone(&task_config), client.http.clone());
        }

        if let Err(e) = client.start().await {
            swarn!(f; "Discord bot error: {}", e);
        }
    });

    shared_config
}

async fn register_discord_subscribers(config: SharedDiscordConfig, http: Arc<Http>) {
    let cfg = config.read().await;

    let broadcast_message_bus = &EVENT_SYSTEM.message_broadcast_event_bus;

    if cfg.general_chat_channel_id.is_some() {
        let discord_subscriber = DiscordBroadcastSubscriber::new(http.clone(), Arc::clone(&config));
        let _ = broadcast_message_bus.subscribe(Box::new(discord_subscriber)).await;
    } else {
        swarn!(f; "Discord chat relay subscriber not registered because general chat channel is not configured. Discord based chat commands will also be disabled.");
    }

    // Admin Alert Module
    if cfg.admin_notification_channel_id.is_some() {
        let admin_alert_module = AdminAlertModule::new(
            http.clone(),
            Arc::clone(&config),
            &EVENT_SYSTEM.game_event_publisher,
        );
        let _ = EVENT_SYSTEM.game_event_bus.subscribe(Box::new(admin_alert_module.clone())).await;

        let command_subscriber = EVENT_SYSTEM.command_subscriber.clone();
        TOKIO_RUNTIME.spawn(async move {
            let mut command_subscriber = command_subscriber.lock().await;
            command_subscriber.register(admin_alert_module);
        });
    } else {
        swarn!(f; "Discord admin alert subscriber not registered because admin notification channel is not configured.");
    }

    // Dashboard Subscriber
    if cfg.dashboard_channel_id.is_some() {
        let dashboard_subscriber = DashboardSubscriber::new(http.clone(), Arc::clone(&config));
        let _ = EVENT_SYSTEM.game_event_bus.subscribe(Box::new(dashboard_subscriber)).await;
    } else {
        swarn!(f; "Discord dashboard subscriber not registered because dashboard channel is not configured.");
    }

    if cfg.event_log_channel_id.is_some() {
        let event_log_subscriber = EventLogSubscriber::new(http.clone(), Arc::clone(&config));
        let _ = EVENT_SYSTEM.game_event_bus.subscribe(Box::new(event_log_subscriber)).await;
    } else {
        swarn!(f; "Discord event log subscriber not registered because event log channel is not configured.");
    }

    // Register Discord Connection with Command Subscriber
    let command_subscriber = EVENT_SYSTEM.command_subscriber.clone();
    let http_clone = http.clone();
    let config_clone = Arc::clone(&config);
    TOKIO_RUNTIME.spawn(async move {
        let mut command_subscriber = command_subscriber.lock().await;
        command_subscriber.set_discord_connection(config_clone, http_clone);
    });
}

fn add_discord_chat_sink(discord_config: SharedDiscordConfig, discord_http: Arc<Http>) {
    TOKIO_RUNTIME.spawn(async move {
        EVENT_SYSTEM.chat_relay_subscriber
            .add_sink(Box::new(DiscordChatSink::new(discord_config, discord_http)))
            .await;
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

        let config = self.config.read().await.clone();

        let is_known_channel =
            config.general_chat_channel_id == Some(msg.channel_id)
            || config.admin_notification_channel_id == Some(msg.channel_id);

        if !is_known_channel {
            return;
        }

        let roles = msg.member.as_ref().map(|m| m.roles.clone()).unwrap_or_default();
        let is_admin = config.admin_role_id.map(|id| roles.contains(&RoleId::new(id))).unwrap_or(false);

        let actor = CommandActor {
            display_name: msg.author.name.clone(),
            identity: ActorIdentity::DiscordUser {
                user_id: msg.author.id,
                display_name: msg.author.display_name().to_string(),
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

            self.event_publisher.publish(GameEvent::CommandRequestEvent(CommandRequest {
                name,
                args,
                raw_args,
                actor,
                source: CommandSource::Discord,
            }));
        } else if config.general_chat_channel_id == Some(msg.channel_id) {
            self.event_publisher.publish(GameEvent::GameChatMessageEvent(GameChatMessage {
                chat_source: ChatSource::Discord,
                chat_type: ChatType::Global,
                sender: msg.author.name.clone(),
                message: msg.content.clone(),
            }));
        }
    }
}
