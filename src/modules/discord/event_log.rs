use std::sync::Arc;
use async_trait::async_trait;
use serenity::all::{CreateEmbed, CreateEmbedFooter, CreateMessage, Http};
use crate::events::bus::Subscriber;
use crate::events::models::GameEvent;
use crate::features::discord::{send_to_channel, SharedDiscordConfig};

pub struct EventLogSubscriber {
    http: Arc<Http>,
    discord_config: SharedDiscordConfig,
}

impl EventLogSubscriber {
    pub fn new(http: Arc<Http>, discord_config: SharedDiscordConfig) -> Self {
        Self {
            http,
            discord_config,
        }
    }

    fn build_embed(event: &GameEvent) -> CreateEmbed {
        let (emoji, title, description, color) = match event {
            GameEvent::JoinEvent(join) => (
                "📥",
                "Player Joined",
                format!("**{}** joined the server.", join.name),
                0x57F287,
            ),
            GameEvent::LeaveEvent(leave) => (
                "📤",
                "Player Left",
                format!("**{}** left the server.", leave.name),
                0xED4245,
            ),
            GameEvent::CrashEvent(crash) => (
                "💀",
                "SERVER CRASH",
                format!(
                    "**{}** \ntrace: \n```\n{}\n```",
                    crash.event_type,
                    crash.event_trace.join("\n")
                ),
                0x992D22,
            ),
            GameEvent::KillEvent(kill) => (
                "⚔️",
                "Kill Event",
                format!(
                    "**Killer:** {}\n**Victim:** {}\n**Weapon:** {}\n**Attack Type:** {}\n**Damage:** {:.2}",
                    kill.killer,
                    kill.victim,
                    kill.source.damage.source,
                    kill.source.damage.attack_type,
                    kill.source.damage.amount
                ),
                0xC53030,
            ),
            GameEvent::MapChangeEvent(map_change) => (
                "🗺️",
                "Map Change",
                format!("Map changed to **{}**.", map_change.new_map_url),
                0x5865F2,
            ),
            GameEvent::MatchEndEvent(match_end) => (
                "🏁",
                "Match End",
                format!(
                    "**Winner:** {}\n**Final Score:** {}",
                    match_end.winner_team, match_end.final_score
                ),
                0xFEE75C,
            ),
            GameEvent::GameChatMessageEvent(chat) => (
                "💬",
                "Game Chat Message",
                format!(
                    "**Source:** {:?}\n**Type:** {:?}\n**Sender:** {}\n**Message:** {}",
                    chat.chat_source, chat.chat_type, chat.sender, chat.message
                ),
                0x1ABC9C,
            ),
            GameEvent::CommandRequestEvent(command) => (
                "📝",
                "Command Requested",
                format!(
                    "**Name:** {}\n**Source:** {:?}\n**Actor:** {}\n**Args:** {}\n**Raw:** `{}`",
                    command.name,
                    command.source,
                    command.actor.display_name,
                    if command.args.is_empty() {
                        "<none>".to_string()
                    } else {
                        command.args.join(" ")
                    },
                    command.raw_args
                ),
                0x3498DB,
            ),
            GameEvent::CommandExecutedEvent(command) => (
                "✅",
                "Command Executed",
                format!(
                    "**Name:** {}\n**Source:** {:?}\n**Actor:** {}\n**Args:** {}\n**Raw:** `{}`",
                    command.name,
                    command.source,
                    command.actor.display_name,
                    if command.args.is_empty() {
                        "<none>".to_string()
                    } else {
                        command.args.join(" ")
                    },
                    command.raw_args
                ),
                0x2ECC71,
            ),
            GameEvent::CommandRejectedEvent(command) => (
                "⛔",
                "Command Rejected",
                format!(
                    "**Name:** {}\n**Source:** {:?}\n**Actor:** {}\n**Args:** {}\n**Raw:** `{}`\n**Reason:** {}",
                    command.name,
                    command.source,
                    command.actor.display_name,
                    if command.args.is_empty() {
                        "<none>".to_string()
                    } else {
                        command.args.join(" ")
                    },
                    command.raw_args,
                    command.rejection_reason
                ),
                0xE74C3C,
            ),
            GameEvent::ServerStatusEvent(status) => (
                "📊",
                "Server Status",
                format!(
                    "**Name:** {}\n**Map:** {}\n**Players:** {}/{}\n**Password Protected:** {}",
                    status.name,
                    status.current_map,
                    status.player_count,
                    status.max_players,
                    status.password_protected
                ),
                0x5DADE2,
            ),
            GameEvent::AdminAlertEvent(alert) => (
                "🚨",
                "Admin Alert",
                format!("**Reporter:** {}\n**Reason:** {}", alert.reporter, alert.reason),
                0xFF6B6B,
            ),
            GameEvent::DuelStartEvent(duel) => (
                "🤺",
                "Duel Start",
                format!("**Challenger:** {}\n**Opponent:** {}", duel.challenger, duel.opponent),
                0x9B59B6,
            ),
            GameEvent::AttackEvent(attack) => (
                "🗡️",
                "Attack Event",
                format!(
                    "**Attacker:** {}\n**Type:** {}\n**Parried:** {}",
                    attack.attacker, attack.attack_type, attack.was_parried
                ),
                0xE67E22,
            ),
            GameEvent::DamageEvent(damage) => (
                "🩸",
                "Damage Event",
                format!(
                    "**Attacker:** {}\n**Victim:** {}\n**Damage:** {:.2}\n**Source:** {}\n**Attack Type:** {}",
                    damage.attacker,
                    damage.victim,
                    damage.damage.amount,
                    damage.damage.source,
                    damage.damage.attack_type
                ),
                0xE74C3C,
            ),
        };

        CreateEmbed::new()
            .title(format!("{} {}", emoji, title))
            .description(description)
            .color(color)
            .footer(CreateEmbedFooter::new("Unchained Event Log"))
    }
}

#[async_trait]
impl Subscriber<GameEvent> for EventLogSubscriber {
    fn identifier(&self) -> &'static str {
        "EventLogSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        let event_log_channel_id = self.discord_config.read().await.event_log_channel_id;
        let message = CreateMessage::new().embed(Self::build_embed(event));
        send_to_channel(&self.http, event_log_channel_id, message).await;
    }
}
