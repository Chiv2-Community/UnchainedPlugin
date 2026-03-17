use async_trait::async_trait;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::{EventPublisher, Subscriber};
use crate::events::models::GameEvent;

pub struct JoinBatcherSubscriber {
    pending_joins: Vec<String>,
    max_batch_size: usize,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl JoinBatcherSubscriber {
    pub fn new(broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self {
            pending_joins: Vec::new(),
            max_batch_size: 10,
            broadcaster,
        }
    }

    fn flush(&mut self) {
        if self.pending_joins.is_empty() {
            return;
        }

        let content = match self.pending_joins.as_slice() {
            [] => return,
            [first] => {
                format!("📥 **{}** joined the fray.", first)
            }
            [first, ..] => {
                format!(
                    "📥 **{}** and **{}** others have joined the battle!",
                    first,
                    self.pending_joins.len() - 1
                )
            }
        };

        self.pending_joins.clear();
        self.broadcaster.publish(BroadcastMessage::from(content));
    }
}

#[async_trait]
impl Subscriber<GameEvent> for JoinBatcherSubscriber {
    fn identifier(&self) -> &'static str {
        "JoinBatcherSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        if let GameEvent::JoinEvent(join) = event {
            self.pending_joins.push(join.name.clone());

            if self.pending_joins.len() >= self.max_batch_size {
                self.flush();
            }
        }
    }

    async fn on_tick(&mut self) {
        self.flush();
    }
}
