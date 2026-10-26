use std::sync::{Arc, OnceLock};
use serenity::all::{ChannelId, Context, CreateMessage, GatewayIntents, Http, Message, RoleId};
use serenity::Client;
use serenity::client::EventHandler as DiscordHandler;
use crate::events::bus::EventPublisher;
use crate::events::models::{ActorIdentity, ActorPermissions, ChatSource, ChatType, CommandActor, CommandSource, GameChatMessage, CommandRequest, GameEvent, PermissionFlags};
use crate::events::broadcast_message::discord::DiscordBroadcastSubscriber;
use crate::modules::chat::DiscordChatSink;
use crate::modules::discord::admin_alert::AdminAlertModule;
use crate::modules::discord::dashboard::DashboardSubscriber;
use crate::modules::discord::event_log::EventLogSubscriber;
use crate::features::events::EVENT_SYSTEM;
use crate::features::tokio_runtime::TOKIO_RUNTIME;
use crate::tools::command_parser::parse_command;

pub struct DiscordConfig {
    pub bot_token: String,
    pub admin_role_id: Option<u64>,
    pub dashboard_channel_id: Option<ChannelId>,
    pub general_chat_channel_id: Option<ChannelId>,
    pub admin_notification_channel_id: Option<ChannelId>,
    pub event_log_channel_id: Option<ChannelId>,
    pub mention_on_admin: bool,
}

pub type SharedDiscordConfig = &'static DiscordConfig;

static DISCORD_CONFIG: OnceLock<DiscordConfig> = OnceLock::new();

fn store_config(
    storage: &OnceLock<DiscordConfig>,
    config: DiscordConfig,
) -> Result<&DiscordConfig, &DiscordConfig> {
    let installed = storage.set(config).is_ok();
    let config = storage
        .get()
        .expect("Discord configuration was initialized");
    if installed {
        Ok(config)
    } else {
        Err(config)
    }
}

pub async fn send_to_channel(
    http: &Http,
    channel_id: Option<ChannelId>,
    message: CreateMessage,
) {
    match channel_id {
        Some(id) => {
            if let Err(e) = id.send_message(http, message).await {
                crate::warn!("Failed to send to channel {}: {}", id, e);
            }
        }
        None => {
            crate::warn!("Attempted to send to a channel that is not configured.");
        }
    }
}

pub struct Discord {
    config: SharedDiscordConfig,
    event_publisher: EventPublisher<GameEvent>,
}

pub fn initialize_discord_system(config: DiscordConfig) -> SharedDiscordConfig {
    let shared_config = match store_config(&DISCORD_CONFIG, config) {
        Ok(config) => config,
        Err(config) => {
            crate::warn!("Discord is already initialized; keeping the original configuration.");
            return config;
        }
    };
    let event_publisher = EVENT_SYSTEM.game_event_publisher.clone();

    TOKIO_RUNTIME.spawn(async move {
        let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

        let mut client = match Client::builder(&shared_config.bot_token, intents)
            .event_handler(Discord {
                config: shared_config,
                event_publisher,
            })
            .await
        {
            Ok(c) => c,
            Err(e) => {
                crate::warn!("Failed to create Discord client: {}", e);
                return;
            }
        };

        register_discord_subscribers(shared_config, client.http.clone()).await;

        if shared_config.general_chat_channel_id.is_some() {
            add_discord_chat_sink(shared_config, client.http.clone()).await;
        }

        if let Err(e) = client.start().await {
            crate::warn!("Discord bot error: {}", e);
        }
    });

    shared_config
}

async fn register_discord_subscribers(config: SharedDiscordConfig, http: Arc<Http>) {
    let broadcast_message_bus = &EVENT_SYSTEM.message_broadcast_event_bus;

    if config.general_chat_channel_id.is_some() {
        let discord_subscriber = DiscordBroadcastSubscriber::new(http.clone(), config);
        let _ = broadcast_message_bus.subscribe(Box::new(discord_subscriber)).await;
    } else {
        crate::warn!("Discord chat relay subscriber not registered because general chat channel is not configured. Discord based chat commands will also be disabled.");
    }

    // Admin Alert Module
    if config.admin_notification_channel_id.is_some() {
        let admin_alert_module = AdminAlertModule::new(
            http.clone(),
            config,
            &EVENT_SYSTEM.game_event_publisher,
        );
        let _ = EVENT_SYSTEM.game_event_bus.subscribe(Box::new(admin_alert_module.clone())).await;

        EVENT_SYSTEM.command_subscriber.lock().await.register(admin_alert_module);
    } else {
        crate::warn!("Discord admin alert subscriber not registered because admin notification channel is not configured.");
    }

    // Dashboard Subscriber
    if config.dashboard_channel_id.is_some() {
        let dashboard_subscriber = DashboardSubscriber::new(http.clone(), config);
        let _ = EVENT_SYSTEM.game_event_bus.subscribe(Box::new(dashboard_subscriber)).await;
    } else {
        crate::warn!("Discord dashboard subscriber not registered because dashboard channel is not configured.");
    }

    if config.event_log_channel_id.is_some() {
        let event_log_subscriber = EventLogSubscriber::new(http.clone(), config);
        let _ = EVENT_SYSTEM.game_event_bus.subscribe(Box::new(event_log_subscriber)).await;
    } else {
        crate::warn!("Discord event log subscriber not registered because event log channel is not configured.");
    }

    // Register Discord Connection with Command Subscriber
    EVENT_SYSTEM.command_subscriber.lock().await.set_discord_connection(config, http);
}

async fn add_discord_chat_sink(discord_config: SharedDiscordConfig, discord_http: Arc<Http>) {
    EVENT_SYSTEM.chat_relay_subscriber
        .add_sink(Box::new(DiscordChatSink::new(discord_config, discord_http)))
        .await;
}

#[async_trait::async_trait]
impl DiscordHandler for Discord {
    async fn ready(&self, _: Context, ready: serenity::model::gateway::Ready) {
        crate::info!("Discord bot user '{}' is connected!", ready.user.name);
    }

    async fn message(&self, _ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let config = self.config;

        let is_known_channel =
            config.general_chat_channel_id == Some(msg.channel_id)
            || config.admin_notification_channel_id == Some(msg.channel_id);

        if !is_known_channel {
            return;
        }

        if msg.content.starts_with('!') {
            let content = &msg.content[1..];
            let Some((name, args, raw_args)) = parse_command(content) else {
                return;
            };

            let roles = msg.member.as_ref().map(|m| m.roles.as_slice()).unwrap_or_default();
            let is_admin = config.admin_role_id.map(|id| roles.contains(&RoleId::new(id))).unwrap_or(false);
            let identity_display_name = msg.author.display_name().to_string();
            let actor = CommandActor {
                display_name: msg.author.name,
                identity: ActorIdentity::DiscordUser {
                    user_id: msg.author.id,
                    display_name: identity_display_name,
                },
                permissions: ActorPermissions {
                    flags: if is_admin { PermissionFlags::ADMIN | PermissionFlags::MODERATOR | PermissionFlags::USER } else { PermissionFlags::USER },
                },
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
                sender: msg.author.name,
                message: msg.content,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(token: &str) -> DiscordConfig {
        DiscordConfig {
            bot_token: token.into(),
            admin_role_id: Some(7),
            dashboard_channel_id: Some(ChannelId::new(10)),
            general_chat_channel_id: Some(ChannelId::new(11)),
            admin_notification_channel_id: Some(ChannelId::new(12)),
            event_log_channel_id: Some(ChannelId::new(13)),
            mention_on_admin: true,
        }
    }

    #[test]
    fn configuration_is_stored_once_and_shared_without_copying() {
        let storage = OnceLock::new();
        let original = store_config(&storage, config("original"))
            .unwrap_or_else(|_| panic!("first initialization must succeed"));
        assert!(std::ptr::eq(original, storage.get().unwrap()));

        let existing = store_config(&storage, config("replacement"))
            .err()
            .expect("duplicate initialization must not start another client");
        assert!(std::ptr::eq(original, existing));
        assert_eq!(existing.bot_token, "original");
    }

    #[test]
    fn concurrent_initialization_has_only_one_winner() {
        let storage = OnceLock::new();
        let barrier = std::sync::Barrier::new(4);
        let winner = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|index| {
                    let storage = &storage;
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        let token = format!("client-{index}");
                        store_config(storage, config(&token)).ok().map(|_| token)
                    })
                })
                .collect();
            let winners: Vec<_> = handles
                .into_iter()
                .filter_map(|handle| handle.join().unwrap())
                .collect();
            assert_eq!(winners.len(), 1);
            winners.into_iter().next().unwrap()
        });
        assert_eq!(storage.get().unwrap().bot_token, winner);
    }
}
