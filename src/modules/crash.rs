use async_trait::async_trait;
use crate::events::broadcast::{BroadcastMessage, Notify};
use crate::events::bus::{EventPublisher, Subscriber};
use crate::events::models::GameEvent;

pub struct CrashSubscriber {
    mention_on_crash: bool,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl CrashSubscriber {
    pub fn new(mention_on_crash: bool, broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self { mention_on_crash, broadcaster }
    }
}

#[async_trait]
impl Subscriber<GameEvent> for CrashSubscriber {
    fn identifier(&self) -> &'static str {
        "CrashSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        if let GameEvent::CrashEvent(alert) = event {
            let mut msg = BroadcastMessage::new()
                .title("🚨 SERVER CRASH".to_string())
                .content(format!("**{}** \ntrace: \n```\n{}\n```", alert.event_type, alert.event_trace.join("\n")));

            if self.mention_on_crash {
                // msg = msg.notify(Notify::Admin);
            }
            self.broadcaster.publish(msg);
        }
    }

    async fn on_tick(&mut self) {}
}
