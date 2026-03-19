use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use serenity::all::{ChannelId, Http, CreateMessage, MessageId, EditMessage};
use crate::events::bus::Subscriber;
use crate::events::models::{GameEvent, ServerStatus};
use crate::features::discord::SharedDiscordConfig;
use crate::swarn;

pub struct DashboardSubscriber {
    // Current State
    player_count: u32,
    max_players: u32,
    current_map: String,
    server_name: String,
    last_update: Instant,
    
    // Discord Reference
    http: Arc<Http>,
    discord_config: SharedDiscordConfig,
    active_channel_id: Option<ChannelId>,
    message_id: Option<MessageId>,
    needs_refresh: bool,
    
    status: Option<ServerStatus>,
}

impl DashboardSubscriber {
    pub fn new(http: Arc<Http>, discord_config: SharedDiscordConfig) -> Self {
        Self {
            player_count: 0,
            max_players: 0,
            current_map: "Loading...".to_string(),
            server_name: "Unchained Server".to_string(),
            last_update: Instant::now(),
            http,
            discord_config,
            active_channel_id: None,
            message_id: None,
            needs_refresh: true,
            status: None,
        }
    }

    fn build_embed(&self) -> serenity::all::CreateEmbed {
        let mut embed = serenity::all::CreateEmbed::new()
            .title(format!("🛡️ {}", self.server_name))
            .color(0x2B2D31)
            .field("Map", &self.current_map, true)
            .field("Players", format!("{}/{}", self.player_count, self.max_players), true)
            .footer(serenity::all::CreateEmbedFooter::new(
                format!("Last updated: {:?}", self.last_update.elapsed())
            ));

        if let Some(status) = &self.status {
            embed = embed.description(&status.description);
            
            let active_mods = if status.active_mods.is_empty() {
                "None".to_string()
            } else {
                status.active_mods
                    .iter()
                    .map(|m| format!("{} *({})*", m.name, m.version))
                    .collect::<Vec<_>>()
                    .join("\n- ")
            };
            embed = embed.field("Active Mods", active_mods, false);
        }

        embed
    }
}

#[async_trait]
impl Subscriber<GameEvent> for DashboardSubscriber {
    fn identifier(&self) -> &'static str {
        "DashboardSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        match event {
            GameEvent::JoinEvent(_) => {
                self.player_count += 1;
                self.needs_refresh = true;
            }
            GameEvent::LeaveEvent(_) => {
                self.player_count = self.player_count.saturating_sub(1);
                self.needs_refresh = true;
            }
            GameEvent::ServerStatusEvent(new_status) => {
                self.status = Some(new_status.clone());
                self.player_count = new_status.player_count as u32;
                self.max_players = new_status.max_players as u32;
                self.current_map = new_status.current_map.clone();
                self.server_name = new_status.name.clone();
                self.needs_refresh = true;
            }
            GameEvent::MapChangeEvent(e) => {
                self.current_map = e.new_map.clone();
                self.needs_refresh = true;
            }
            _ => {}
        }
    }

    async fn on_tick(&mut self) {
        if !self.needs_refresh && self.message_id.is_none() {
            return;
        }

        if !self.needs_refresh && self.last_update.elapsed() < Duration::from_secs(30) {
            return;
        }

        let channel_id = match self.discord_config.read().await.dashboard_channel_id {
            Some(id) => id,
            None => {
                swarn!(f; "Attempted to update dashboard, but dashboard channel is not configured.");
                return;
            }
        };

        if self.active_channel_id != Some(channel_id) {
            self.active_channel_id = Some(channel_id);
            self.message_id = None;
            self.needs_refresh = true;
        }

        let embed = self.build_embed();

        match self.message_id {
            Some(id) => {
                let _ = channel_id.edit_message(&self.http, id, EditMessage::new().add_embed(embed)).await;
            }
            None => {
                if let Ok(msg) = channel_id.send_message(&self.http, CreateMessage::new().add_embed(embed)).await {
                    self.message_id = Some(msg.id);
                }
            }
        }

        self.last_update = Instant::now();
        self.needs_refresh = false;
    }
}
