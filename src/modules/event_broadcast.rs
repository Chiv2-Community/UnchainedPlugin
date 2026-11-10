use async_trait::async_trait;
use crate::events::broadcast::{BroadcastMessage, RichText};
use crate::events::bus::{EventPublisher, Subscriber};
use crate::events::models::GameEvent;

pub struct EventBroadcastSubscriber {
    pending_joins: Vec<String>,
    pending_leaves: Vec<String>,
    max_batch_size: usize,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl EventBroadcastSubscriber {
    pub fn new(broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self {
            pending_joins: Vec::new(),
            pending_leaves: Vec::new(),
            max_batch_size: 10,
            broadcaster,
        }
    }

    fn flush_batch(&self, pending_players: &[String], action: PresenceAction) -> Option<RichText> {
        if pending_players.is_empty() {
            return None;
        }

        let (icon, single, multiple) = match action {
            PresenceAction::Joined => ("📥 ", " joined the fray.", " others have joined the battle!"),
            PresenceAction::Left => ("📤 ", " left the battle.", " others have left the battle!"),
        };
        let content = if pending_players.len() == 1 {
            crate::rich_text!("{}**{}**{}", icon, pending_players[0], single)
        } else {
            crate::rich_text!("{}**{}** and **{}**{}", icon, pending_players[0], pending_players.len() - 1, multiple)
        };
        Some(content)
    }

    fn flush_joins(&mut self) {
        let content = self.flush_batch(self.pending_joins.as_slice(), PresenceAction::Joined);
        self.pending_joins.clear();
        if let Some(message) = content {
            self.broadcaster.publish(BroadcastMessage::from(message));
        }
    }

    fn flush_leaves(&mut self) {
        let content = self.flush_batch(self.pending_leaves.as_slice(), PresenceAction::Left);
        self.pending_leaves.clear();
        if let Some(message) = content {
            self.broadcaster.publish(BroadcastMessage::from(message));
        }
    }

    fn flush_all(&mut self) {
        self.flush_joins();
        self.flush_leaves();
    }
}

#[derive(Debug, Clone, Copy)]
enum PresenceAction {
    Joined,
    Left,
}

static mut SERVER_STARTED: bool = false;

#[async_trait]
impl Subscriber<GameEvent> for EventBroadcastSubscriber {
    fn identifier(&self) -> &'static str {
        "EventBroadcastSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        match event {
            GameEvent::JoinEvent(join) => {
                self.pending_joins.push(join.name.clone());
                if self.pending_joins.len() >= self.max_batch_size {
                    self.flush_joins();
                }
            }
            GameEvent::LeaveEvent(leave) => {
                self.pending_leaves.push(leave.name.clone());
                if self.pending_leaves.len() >= self.max_batch_size {
                    self.flush_leaves();
                }
            }
            GameEvent::MapChangeEvent(map_change) => {
                let new_map_name = map_change.new_map_name.clone();

                if !unsafe { SERVER_STARTED } {
                    self.broadcaster.publish(
                        BroadcastMessage::new()
                            .title(crate::rich_text!("🚀️ Server Started"))
                            .content(crate::rich_text!("Map {}.", new_map_name))
                            .color(0x5865F2)
                    );
                    unsafe { SERVER_STARTED = true };
                } else {
                    self.broadcaster.publish(
                        BroadcastMessage::new()
                            .title(crate::rich_text!("🗺️ Map Change"))
                            .content(crate::rich_text!("Map changed to {}.", new_map_name))
                            .color(0x5865F2)
                    );
                }
            },
            GameEvent::MatchEndEvent(match_end) => {
                self.broadcaster.publish(
                    BroadcastMessage::new()
                        .title(crate::rich_text!("🏁 Match Ended"))
                        .field(crate::rich_text!("Winner"), crate::rich_text!("{}", match_end.winner_team))
                        .field(crate::rich_text!("Score"), crate::rich_text!("{}", match_end.final_score))
                        .color(0x5865F2)
                );
            },
            GameEvent::ServerRestartEvent => {
                self.broadcaster.publish(
                    BroadcastMessage::new()
                        .title(crate::rich_text!("🔄 Server Restart"))
                        .content(crate::rich_text!("Server is restarting"))
                        .color(0xE67E22)
                );
            }
            _ => {}
        }
    }

    async fn on_tick(&mut self) {
        self.flush_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn presence_templates_preserve_text_and_emphasis() {
        let (broadcaster, _) = crate::test_utils::mock_broadcaster();
        let subscriber = EventBroadcastSubscriber::new(broadcaster);
        assert!(subscriber.flush_batch(&[], PresenceAction::Joined).is_none());
        let players = vec!["Alice".into(), "Bob".into()];
        for (count, action, expected) in [
            (1, PresenceAction::Joined, "📥 **Alice** joined the fray."),
            (1, PresenceAction::Left, "📤 **Alice** left the battle."),
            (2, PresenceAction::Joined, "📥 **Alice** and **1** others have joined the battle!"),
            (2, PresenceAction::Left, "📤 **Alice** and **1** others have left the battle!"),
        ] {
            let text = subscriber.flush_batch(&players[..count], action).unwrap();
            assert_eq!(text.to_discord(), expected);
            assert_eq!(text.to_plain_text(), expected.replace("**", ""));
        }
    }
}
