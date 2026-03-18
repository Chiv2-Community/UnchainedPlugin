use async_trait::async_trait;
use std::collections::HashMap;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::{EventPublisher, Subscriber};
use crate::events::models::GameEvent;

pub struct KillstreakSubscriber {
    streaks: HashMap<String, u32>,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl KillstreakSubscriber {
    pub fn new(broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self {
            streaks: HashMap::new(),
            broadcaster,
        }
    }

    fn build_message(&self, player: &str, message: &str, color: u32) -> BroadcastMessage {
        BroadcastMessage::new()
            .title("?? Killstreak".to_string())
            .content(format!("**{}** {}", player, message))
            .color(color)
    }
}

#[async_trait]
impl Subscriber<GameEvent> for KillstreakSubscriber {
    fn identifier(&self) -> &'static str {
        "KillstreakSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        if let GameEvent::KillEvent(kill) = event {
            // 1. Reset the victim's streak
            self.streaks.remove(&kill.victim);

            // 2. Increment the killer's streak
            let entry = self.streaks.entry(kill.killer.clone()).or_insert(0);
            *entry += 1;
            let current_streak = *entry;

            // 3. Only publish on milestones
            let msg = match current_streak {
                5 => Some(self.build_message(&kill.killer, "is on a Killing Spree!", 0x3498db)),
                10 => Some(self.build_message(&kill.killer, "is UNSTOPPABLE!", 0x9b59b6)),
                15 => Some(self.build_message(&kill.killer, "is GODLIKE!", 0xe74c3c)),
                20 => Some(self.build_message(&kill.killer, "is a LEGENDARY WARRIOR!", 0xf1c40f)),
                _ => None,
            };

            if let Some(msg) = msg {
                self.broadcaster.publish(msg);
            }
        }
    }

    async fn on_tick(&mut self) {}
}
