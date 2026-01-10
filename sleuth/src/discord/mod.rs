pub mod core;
pub mod modules;
pub mod notifications;
pub mod responses;
#[macro_use]
pub mod macros;

use crate::discord::core::*;
use crate::discord::modules::chat_relay::ChatRelayModule;
use crate::discord::modules::map_vote::ExtMapVote;
use crate::discord::modules::{
    batcher::JoinBatcher, dashboard::Dashboard, duel_manager::DuelManager, herald::AdminHerald,
    killstreak::KillstreakModule, stats_tracker::StatsTracker,
};
use crate::discord::notifications::{CommandRequest, GameChatMessage};
use crate::discord::responses::{BotResponse, ResponseContent, Target};
use crate::game::chivalry2::EChatType;
use crate::game::engine::FText;
use crate::ue::FString;
use crate::{sinfo, swarn};
use censor::Censor;
use crossbeam_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use serenity::all::{CreateMessage, Http};
use serenity::model::prelude::*;
use serenity::prelude::*;
use std::sync::{Arc, OnceLock};
use crate::resolvers::admin_control::o_FText_AsCultureInvariant;
use crate::resolvers::messages::o_BroadcastLocalizedChat;
use crate::resolvers::etc_hooks::o_GetTBLGameMode;

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct DiscordConfig {
//     pub bot_token: String,
//     pub channel_id: u64,
//     pub admin_role_id: u64,

//     #[serde(default = "default_modules")]
//     pub enabled_modules: Vec<String>,

//     #[serde(default = "default_filters")]
//     pub notification_filter: Vec<String>,

//     #[serde(default = "default_true")]
//     pub enable_dashboard: bool,
// }

// fn default_modules() -> Vec<String> {
//     vec![
//         "JoinBatcher".into(),
//         "Dashboard".into(),
//         "AdminHerald".into(),
//         "KillStreak".into(),
//         "DuelManager".into(),
//         "StatsTracker".into(),
//     ]
// }

// fn default_filters() -> Vec<String> {
//     vec![
//         "Join".into(),
//         "Chat".into(),
//         "AdminAlert".into(),
//         "MatchWon".into(),
//     ]
// }

#[derive(serde::Deserialize, Clone, Debug)]
pub struct DiscordConfig {
    pub bot_token: String,
    pub channel_id: u64,
    pub admin_channel_id: u64,
    pub general_channel_id: u64,
    pub admin_role_id: u64,
    pub disabled_modules: Vec<String>,
    pub blocked_notifications: Vec<String>,
}

fn default_true() -> bool {
    true
}

pub static DISCORD_HANDLE: OnceLock<DiscordHandle> = OnceLock::new();

// #[derive(Clone, Debug)]
// pub struct DiscordHandle {
//     sender: Sender<Box<dyn GameEvent>>,
// }

// impl DiscordHandle {
//     pub fn dispatch<T: GameEvent>(&self, event: T) {
//         // Non-blocking send to ensure the game thread never hitches.
//         let _ = self.sender.try_send(Box::new(event));
//     }
// }
#[derive(Clone, Debug)]
pub struct DiscordHandle {
    // Change Sender to tokio's version
    pub sender: tokio::sync::mpsc::UnboundedSender<Box<dyn GameEvent>>,
}

impl DiscordHandle {
    pub fn dispatch<E: GameEvent + 'static>(&self, event: E) {
        let _ = self.sender.send(Box::new(event));
    }
}

/// A special internal module that handles events that are
/// "Self-Reporting" notifications.
struct SimpleNotifier;
#[async_trait::async_trait]
impl DiscordSubscriber for SimpleNotifier {
    fn name(&self) -> &'static str {
        "SimpleNotifier"
    }
    async fn on_event(
        &mut self,
        event: &dyn GameEvent,
        _: &Arc<Http>,
        _: ChannelId,
    ) -> Vec<BotResponse> {
        // event.to_notification()
        vec![]
    }
}

pub fn send_ingame_message(message: String, chat_type: Option<EChatType>) {
    let chat_type_actual = chat_type.unwrap_or(EChatType::AllSay);
    if let Some(world) = crate::globals().world() {
        let mut settings_fstring = FString::from(message.as_str());
        let mut txt = FText::default();

        unsafe {
            let res = TRY_CALL_ORIGINAL!(FText_AsCultureInvariant(&mut txt, &mut settings_fstring));

            let game_mode = TRY_CALL_ORIGINAL!(GetTBLGameMode(world));

            if !game_mode.is_null() {
                TRY_CALL_ORIGINAL!(BroadcastLocalizedChat(game_mode, res, chat_type_actual));
            }
        }
    }
}

pub struct DiscordBridge;

async fn dispatch_responses(
    http: &Arc<Http>,
    responses: Vec<BotResponse>,
    main_channel: ChannelId,
    admin_channel: ChannelId,
    general_channel: ChannelId,
    // Add other channels here as needed
) {
    for resp in responses {
        // Resolve targets: if empty, default to Main
        let targets = if resp.targets.is_empty() {
            vec![Target::Main]
        } else {
            resp.targets
        };

        for target in targets {
            let target_id = match target {
                Target::Main => main_channel,
                Target::Admin => admin_channel,
                Target::General => general_channel,
                Target::Custom(id) => id,
            };

            match &resp.content {
                ResponseContent::Message(m) => {
                    let _ = target_id.send_message(http, m.clone()).await;
                }
                ResponseContent::Embed(e) => {
                    let m = CreateMessage::new().embed(e.clone());
                    let _ = target_id.send_message(http, m).await;
                }
            }
        }
    }
}

fn sanitize_text(input: &str) -> String {
    let filter = Censor::Standard;
    filter.censor(input)
}

impl DiscordBridge {
    pub fn init(config: DiscordConfig) -> DiscordHandle {
        // let (tx, rx) = bounded::<Box<dyn GameEvent>>(1000);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Box<dyn GameEvent>>();
        let cfg = config.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // 1. Initialize all available modules
                // In your init function
                let all_subscribers: Vec<Box<dyn DiscordSubscriber>> = vec![
                    Box::new(SimpleNotifier),
                    Box::new(Dashboard::new()),
                    Box::new(JoinBatcher::default()),
                    Box::new(AdminHerald::new(cfg.admin_role_id)),
                    // Box::new(KillstreakModule::new()), // Not yet implemented (events)
                    // Box::new(DuelManager::new()), // Not yet implemented (events)
                    // Box::new(StatsTracker::new("discord/leaderboard.json")), // Not yet implemented (events)
                    Box::new(ChatRelayModule::new()),
                    Box::new(ExtMapVote::new()),
                ];

                // 2. Filter modules based on config names
                let mut active_subs: Vec<Box<dyn DiscordSubscriber>> = all_subscribers
                    .into_iter()
                    .filter(|s| s.name() == "SimpleNotifier" || !cfg.disabled_modules.contains(&s.name().to_string()))
                    .collect();
                
                swarn!(f; "Active subs: {}, disabled: {:?}", active_subs.len(), cfg.disabled_modules);
                for sub in &active_subs {
                    swarn!(f; "Active: {}", sub.name());
                }

                let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
                let mut client = Client::builder(&cfg.bot_token, intents)
                    .event_handler(Handler { config: cfg.clone() })
                    .await
                    .expect("Failed to create Discord client");

                let http = Arc::clone(&client.http);
                let channel_id = ChannelId::new(cfg.channel_id);
                let admin_channel_id = ChannelId::new(cfg.admin_channel_id);
                let general_channel_id = ChannelId::new(cfg.general_channel_id);
                let blocked_set: std::collections::HashSet<String> = cfg.blocked_notifications.into_iter().collect();

                // 3. The Dispatch Loop
                tokio::spawn(async move {
                    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(1));
                    loop {
                        tokio::select! {
                            Some(mut event) = rx.recv() => {
                                event.sanitize();

                                for sub in &mut active_subs {
                                    // Get the responses (one or many)
                                    let responses = sub.on_event(event.as_ref(), &http, channel_id).await;
                                    dispatch_responses(&http, responses, channel_id, admin_channel_id, general_channel_id).await;
                                }
                            }
                            // Some(event) = rx.recv() => {
                            //     sinfo!(f; "Got event {:#?}", event.event_type());

                            //     if blocked_set.contains(event.event_type()) { continue; }

                            //     for sub in &mut active_subs {
                            //         if let Some(msg) = sub.on_event(event.as_ref(), &http, channel_id).await {
                            //             let _ = channel_id.send_message(&http, msg).await;
                            //         }
                            //     }
                            // }
                            _ = ticker.tick() => {
                                for sub in &mut active_subs {
                                    if let resps = sub.on_tick(&http, channel_id).await {
                                        dispatch_responses(&http, resps, channel_id, admin_channel_id, general_channel_id).await;
                                    }
                                }
                            }
                        }
                    }
                });

                client.start().await.expect("Client crash");
            });
        });

        DiscordHandle { sender: tx }
    }
}

// Minimal Handler to bridge Discord Chat -> System
struct Handler {
    config: DiscordConfig,
}
#[async_trait::async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, msg: Message) {
        // sinfo!(f; "Got message: {}", msg.content.clone());
        if msg.author.bot || msg.channel_id.get() != self.config.channel_id {
            // swarn!(f; "Bot message or id mismatch");
            return;
        }

        if let Some(handle) = DISCORD_HANDLE.get() {
            let req = CommandRequest {
                command: msg.content.clone(),
                user: msg.author.name.clone(),
                // Get roles from the member object (requires GatewayIntents::GUILD_MEMBERS)
                user_roles: msg
                    .member
                    .as_ref()
                    .map(|m| m.roles.clone())
                    .unwrap_or_default(),
            };
            handle.dispatch(CommandRequest {
                command: msg.content.clone(),
                user: msg.author.name.clone(),
                // Get roles from the member object (requires GatewayIntents::GUILD_MEMBERS)
                user_roles: msg
                    .member
                    .as_ref()
                    .map(|m| m.roles.clone())
                    .unwrap_or_default(),
            });
            swarn!(f; "Dispatched command: {:#?}", req);
        }
        else {
            swarn!(f; "No discord handle");
        }
    }
    // async fn message(&self, _ctx: Context, msg: Message) {
    //     if msg.author.bot || msg.channel_id.get() != self.config.channel_id { return; }
    //     if let Some(handle) = DISCORD_HANDLE.get() {
    //         // Dispatch a CommandRequest event from Discord
    //         // (You'll define this struct in notifications.rs)
    //     }
    // }
}
