//! Best-effort Discord event logging, isolated from the game event bus.
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
use crate::events::models::{CombatActor, CommandSource, GameEvent};
use crate::features::discord::{send_to_channel, SharedDiscordConfig};
use async_trait::async_trait;
use serenity::all::{CreateEmbed, CreateEmbedFooter, CreateMessage, Http};
use std::fmt::{self, Display};
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

struct ActorDisplay<'a>(&'a CombatActor);

impl Display for ActorDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_bot {
            f.write_str("(bot) ")?;
        }
        f.write_str(&self.0.name)
    }
}

struct ActorsDisplay<'a>(&'a [CombatActor]);

impl Display for ActorsDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, actor) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", ActorDisplay(actor))?;
        }
        Ok(())
    }
}

struct ArgsDisplay<'a>(&'a [String]);

impl Display for ArgsDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("<none>");
        }
        for (index, arg) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            f.write_str(arg)?;
        }
        Ok(())
    }
}

fn command_description(
    name: &str,
    source: &CommandSource,
    actor: &str,
    args: &[String],
    raw_args: &str,
) -> String {
    format!(
        "**Name:** {}\n**Source:** {:?}\n**Actor:** {}\n**Args:** {}\n**Raw:** `{}`",
        name,
        source,
        actor,
        ArgsDisplay(args),
        raw_args
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
                    ActorDisplay(&kill.killer),
                    ActorDisplay(&kill.victim),
                    ActorsDisplay(&kill.killers),
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
                    &command.args,
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
                    &command.args,
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
                        &command.args,
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
                    ActorDisplay(&damage.attacker_actor),
                    ActorDisplay(&damage.victim_actor),
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
            crate::swarn!(
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
    fn command_formatting_preserves_all_three_event_descriptions() {
        use crate::events::models::{
            CommandExecuted, CommandRejected, CommandRequest, CommandSource,
        };
        let actor = crate::test_utils::mock_actor("Admin");
        for args in [
            vec![],
            vec!["hello".into(), "🩸".into()],
            vec![String::new()],
        ] {
            let expected = format!(
                "**Name:** say\n**Source:** ServerConsole\n**Actor:** Admin\n**Args:** {}\n**Raw:** `say hello`",
                if args.is_empty() { "<none>".into() } else { args.join(" ") },
            );
            let events = [
                GameEvent::CommandRequestEvent(CommandRequest {
                    name: "say".into(),
                    source: CommandSource::ServerConsole,
                    actor: actor.clone(),
                    args: args.clone(),
                    raw_args: "say hello".into(),
                }),
                GameEvent::CommandExecutedEvent(CommandExecuted {
                    name: "say".into(),
                    source: CommandSource::ServerConsole,
                    actor: actor.clone(),
                    args: args.clone(),
                    raw_args: "say hello".into(),
                }),
                GameEvent::CommandRejectedEvent(CommandRejected {
                    name: "say".into(),
                    source: CommandSource::ServerConsole,
                    actor: actor.clone(),
                    args,
                    raw_args: "say hello".into(),
                    rejection_reason: "Denied".into(),
                }),
            ];
            for event in events {
                let text = description(&EventLogSubscriber::build_embed(&event).embed);
                if matches!(event, GameEvent::CommandRejectedEvent(_)) {
                    assert_eq!(text, format!("{}\n**Reason:** Denied", expected));
                } else {
                    assert_eq!(text, expected);
                }
            }
        }
    }

    #[test]
    fn killer_list_display_preserves_empty_single_and_mixed_actor_lists() {
        let cases = [
            (vec![], ""),
            (vec![CombatActor::new("Knight".into(), false)], "Knight"),
            (
                vec![CombatActor::new("Bot 🩸".into(), true)],
                "(bot) Bot 🩸",
            ),
            (
                vec![
                    CombatActor::new("Knight".into(), false),
                    CombatActor::new("Bot 🩸".into(), true),
                ],
                "Knight, (bot) Bot 🩸",
            ),
        ];
        for (actors, expected) in cases {
            assert_eq!(ActorsDisplay(&actors).to_string(), expected);
        }
    }

    #[test]
    fn combat_formatting_preserves_bot_labels_and_kill_details() {
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
        assert_eq!(text, "**Attacker:** Knight\n**Victim:** (bot) Training Bot\n**Damage:** 42.50\n**Source:** Longsword\n**Attack Type:** Backstab");
        let kill = Kill {
            killer: killer.clone(),
            victim: victim.clone(),
            killers: vec![killer, victim],
            kill_reason: "Damage".into(),
            random_seed: 0,
            dead_character_id: 0,
            attach_to_projectile: true,
            source: damage,
        };
        let text = description(&EventLogSubscriber::build_embed(&GameEvent::KillEvent(kill)).embed);
        assert_eq!(text, "**Killer:** Knight\n**Victim:** (bot) Training Bot\n**All Killers:** Knight, (bot) Training Bot\n**Kill Reason:** Damage\n**Weapon:** Longsword\n**Attack Type:** Backstab\n**Damage:** 42.50\n**Attach To Projectile:** true");
    }
}
