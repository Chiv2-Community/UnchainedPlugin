//! Posts a human-readable log of game events to a configured Discord channel.
//!
//! The subscriber queues up to 256 events without waiting for Discord. One worker
//! formats and sends them in order, batching at most 10 embeds / 6000 UTF-16 units.
//! Partial batches flush five seconds after their first event, independently of
//! bus ticks. Descriptions exceeding 4096 units are truncated with an ellipsis.
//!
//! On overload, new events are dropped and counted in warnings at most once every
//! five seconds. Failed sends retain the shared Discord helper's log-and-discard
//! behavior (no retry). Unsubscribing closes the queue; the worker drains and
//! flushes it while the runtime remains alive. Process exit cannot guarantee delivery.

use crate::events::bus::Subscriber;
use crate::events::models::{Args, CommandSource, GameEvent};
use crate::features::discord::{send_to_channel, SharedDiscordConfig};
use async_trait::async_trait;
use itertools::Itertools;
use serenity::all::{CreateEmbed, CreateEmbedFooter, CreateMessage, Http};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

mod worker;

const QUEUE_CAPACITY: usize = 256;
const FOOTER: &str = "Unchained Event Log";
const DROP_REPORT_INTERVAL: Duration = Duration::from_secs(5);

/// Discord I/O and formatting never run under the game event bus's subscriber lock.
/// On overload, discard new log events rather than block commands and gameplay subscribers.
pub struct EventLogSubscriber {
    sender: mpsc::Sender<GameEvent>,
    dropped_events: usize,
    last_drop_report: Instant,
}

fn command_description(
    name: &str,
    source: &CommandSource,
    actor: &str,
    args: Args<'_>,
    raw_args: &str,
) -> String {
    format!(
        "**Name:** {}\n**Source:** {:?}\n**Actor:** {}\n**Args:** {}\n**Raw:** `{}`",
        name, source, actor, args, raw_args
    )
}

impl EventLogSubscriber {
    pub fn new(http: Arc<Http>, discord_config: SharedDiscordConfig) -> Self {
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let handle = tokio::runtime::Handle::try_current()
            .unwrap_or_else(|_| crate::features::tokio_runtime::TOKIO_RUNTIME_HANDLE.clone());
        handle.spawn(async move {
            // Own the handles for the worker's lifetime; each send only borrows them.
            worker::run(receiver, |embeds| {
                let http = &http;
                async move {
                    let channel_id = discord_config.event_log_channel_id;
                    if channel_id.is_some() {
                        send_to_channel(http, channel_id, CreateMessage::new().embeds(embeds))
                            .await;
                    }
                }
            })
            .await;
        });
        Self {
            sender,
            dropped_events: 0,
            last_drop_report: Instant::now(),
        }
    }

    fn build_embed(event: &GameEvent) -> worker::LogEmbed {
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
                    "**Killer:** {}\n**Victim:** {}\n**All Killers:** {}\n**Kill Reason:** {}\n**Weapon:** {}\n**Attack Type:** {}\n**Damage:** {:.2}\n**Attach To Projectile:** {}",
                    kill.killer,
                    kill.victim,
                    kill.killers.iter().format(", "),
                    kill.kill_reason,
                    kill.source.damage.source,
                    kill.source.damage.attack_type,
                    kill.source.damage.amount,
                    kill.attach_to_projectile
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
                command_description(
                    &command.name,
                    &command.source,
                    &command.actor.display_name,
                    Args(&command.args),
                    &command.raw_args,
                ),
                0x3498DB,
            ),
            GameEvent::CommandExecutedEvent(command) => (
                "✅",
                "Command Executed",
                command_description(
                    &command.name,
                    &command.source,
                    &command.actor.display_name,
                    Args(&command.args),
                    &command.raw_args,
                ),
                0x2ECC71,
            ),
            GameEvent::CommandRejectedEvent(command) => (
                "⛔",
                "Command Rejected",
                {
                    let mut description = command_description(
                        &command.name,
                        &command.source,
                        &command.actor.display_name,
                        Args(&command.args),
                        &command.raw_args,
                    );
                    description.push_str("\n**Reason:** ");
                    description.push_str(&command.rejection_reason);
                    description
                },
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
                format!(
                    "**Reporter:** {}\n**Reason:** {}",
                    alert.reporter, alert.reason
                ),
                0xFF6B6B,
            ),
            GameEvent::DuelStartEvent(duel) => (
                "🤺",
                "Duel Start",
                format!(
                    "**Challenger:** {}\n**Opponent:** {}",
                    duel.challenger, duel.opponent
                ),
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
                    damage.attacker_actor,
                    damage.victim_actor,
                    damage.damage.amount,
                    damage.damage.source,
                    damage.damage.attack_type
                ),
                0xE74C3C,
            ),
            GameEvent::ServerRestartEvent => (
                "🔄",
                "Restart Server Event",
                "Server is restarting".to_string(),
                0xE67E22,
            ),
        };

        let title = format!("{} {}", emoji, title);
        let (description, description_units) = worker::truncate_description(description);
        let characters =
            title.encode_utf16().count() + description_units + FOOTER.encode_utf16().count();
        worker::LogEmbed {
            embed: CreateEmbed::new()
                .title(title)
                .description(description)
                .color(color)
                .footer(CreateEmbedFooter::new(FOOTER)),
            characters,
        }
    }
}

#[async_trait]
impl Subscriber<GameEvent> for EventLogSubscriber {
    fn identifier(&self) -> &'static str {
        "EventLogSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        // Reserve first so overload does not clone an event that will be discarded.
        match self.sender.try_reserve() {
            Ok(permit) => permit.send(event.clone()),
            Err(_) => self.dropped_events = self.dropped_events.saturating_add(1),
        }
    }

    async fn on_tick(&mut self) {
        if self.dropped_events > 0 && self.last_drop_report.elapsed() >= DROP_REPORT_INTERVAL {
            crate::warn!(
                "Discord event log dropped {} events: worker queue full or closed",
                self.dropped_events
            );
            self.dropped_events = 0;
            self.last_drop_report = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn description(embed: &CreateEmbed) -> String {
        serde_json::to_value(embed).unwrap()["description"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn command_embeds_include_command_information() {
        use crate::events::models::{
            CommandExecuted, CommandRejected, CommandRequest, CommandSource,
        };
        let actor = crate::test_utils::mock_actor("Admin");
        for args in [
            vec![],
            vec!["first-arg".into(), "second-arg 🩸".into()],
            vec![String::new()],
        ] {
            let events = [
                GameEvent::CommandRequestEvent(CommandRequest {
                    name: "test-command".into(),
                    source: CommandSource::ServerConsole,
                    actor: actor.clone(),
                    args: args.clone(),
                    raw_args: "raw-input".into(),
                }),
                GameEvent::CommandExecutedEvent(CommandExecuted {
                    name: "test-command".into(),
                    source: CommandSource::ServerConsole,
                    actor: actor.clone(),
                    args: args.clone(),
                    raw_args: "raw-input".into(),
                }),
                GameEvent::CommandRejectedEvent(CommandRejected {
                    name: "test-command".into(),
                    source: CommandSource::ServerConsole,
                    actor: actor.clone(),
                    args: args.clone(),
                    raw_args: "raw-input".into(),
                    rejection_reason: "Denied".into(),
                }),
            ];
            for event in events {
                let text = description(&EventLogSubscriber::build_embed(&event).embed);
                for value in ["test-command", "ServerConsole", "Admin", "raw-input"] {
                    assert!(text.contains(value), "missing {value:?} in {text:?}");
                }
                for arg in args.iter().filter(|arg| !arg.is_empty()) {
                    assert!(text.contains(arg), "missing {arg:?} in {text:?}");
                }
                if matches!(event, GameEvent::CommandRejectedEvent(_)) {
                    assert!(
                        text.contains("Denied"),
                        "missing rejection reason in {text:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn combat_embeds_include_damage_and_kill_information() {
        use crate::events::models::{CombatActor, Damage, DamageSource, Kill};
        let killer = CombatActor::new("Knight".into(), false);
        let victim = CombatActor::new("Training Bot".into(), true);
        let damage = Damage {
            attacker: killer.name.clone(),
            victim: victim.name.clone(),
            attacker_actor: killer.clone(),
            victim_actor: victim.clone(),
            damage: DamageSource {
                amount: 42.5,
                source: "Longsword".into(),
                attack_type: "Backstab".into(),
            },
        };
        let text = description(
            &EventLogSubscriber::build_embed(&GameEvent::DamageEvent(damage.clone())).embed,
        );
        for value in ["Knight", "Training Bot", "42.5", "Longsword", "Backstab"] {
            assert!(text.contains(value), "missing {value:?} in {text:?}");
        }
        let assistant = CombatActor::new("Assistant 🩸".into(), false);
        for killers in [
            vec![],
            vec![assistant.clone()],
            vec![victim.clone()],
            vec![assistant.clone(), victim.clone()],
        ] {
            let kill = Kill {
                killer: killer.clone(),
                victim: victim.clone(),
                killers: killers.clone(),
                kill_reason: "Fatal hit".into(),
                random_seed: 0,
                dead_character_id: 0,
                attach_to_projectile: true,
                source: damage.clone(),
            };
            let text =
                description(&EventLogSubscriber::build_embed(&GameEvent::KillEvent(kill)).embed);
            for value in [
                "Knight",
                "Training Bot",
                "Fatal hit",
                "Longsword",
                "Backstab",
                "42.5",
                "true",
            ] {
                assert!(text.contains(value), "missing {value:?} in {text:?}");
            }
            for actor in killers {
                assert!(
                    text.contains(&actor.name),
                    "missing {:?} in {text:?}",
                    actor.name
                );
            }
        }
    }
}
