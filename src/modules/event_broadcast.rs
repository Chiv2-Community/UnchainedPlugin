use async_trait::async_trait;
use crate::events::broadcast::BroadcastMessage;
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

    fn flush_batch(&self, pending_players: &[String], action: PresenceAction) -> Option<String> {
        if pending_players.is_empty() {
            return None;
        }

        let content = match pending_players {
            [] => return None,
            [first] => {
                match action {
                    PresenceAction::Joined => format!("📥 **{}** joined the fray.", first),
                    PresenceAction::Left => format!("📤 **{}** left the battle.", first),
                }
            }
            [first, ..] => {
                let others = pending_players.len() - 1;
                match action {
                    PresenceAction::Joined => format!("📥 **{}** and **{}** others have joined the battle!", first, others),
                    PresenceAction::Left => format!("📤 **{}** and **{}** others have left the battle!", first, others),
                }
            }
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
                self.broadcaster.publish(
                    BroadcastMessage::new()
                        .title("🗺️ Map Change")
                        .content(format!("Map changed to {}.", new_map_name))
                        .color(0x5865F2)
                );
            },
            GameEvent::MatchEndEvent(match_end) => {
                self.broadcaster.publish(
                    BroadcastMessage::new()
                        .title("🏁 Match Ended")
                        .field("Winner", match_end.winner_team.to_string())
                        .field("Score", match_end.final_score.to_string())
                        .color(0x5865F2)
                );
            }
            _ => {}
        }
    }

    async fn on_tick(&mut self) {
        self.flush_all();
    }
}
