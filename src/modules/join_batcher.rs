use async_trait::async_trait;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::{EventPublisher, Subscriber};
use crate::events::models::GameEvent;

pub struct JoinBatcherSubscriber {
    pending_joins: Vec<String>,
    pending_leaves: Vec<String>,
    max_batch_size: usize,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl JoinBatcherSubscriber {
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
impl Subscriber<GameEvent> for JoinBatcherSubscriber {
    fn identifier(&self) -> &'static str {
        "JoinBatcherSubscriber"
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
            _ => {}
        }
    }

    async fn on_tick(&mut self) {
        self.flush_all();
    }
}
