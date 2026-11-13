use async_trait::async_trait;
use crate::events::broadcast::{BroadcastMessage, RichText};
use crate::events::bus::{EventPublisher, Subscriber};
use crate::events::models::GameEvent;

const MAX_BATCH_SIZE: usize = 10;

pub struct EventBroadcastSubscriber {
    pending_joins: Vec<String>,
    pending_leaves: Vec<String>,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl EventBroadcastSubscriber {
    pub fn new(broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self {
            pending_joins: Vec::new(),
            pending_leaves: Vec::new(),
            broadcaster,
        }
    }

    fn presence_message(pending_players: &[String], action: PresenceAction) -> Option<RichText> {
        let (first, others) = pending_players.split_first()?;
        Some(match (action, others.is_empty()) {
            (PresenceAction::Joined, true) => crate::rich_text!("📥 **{}** joined the fray.", first),
            (PresenceAction::Left, true) => crate::rich_text!("📤 **{}** left the battle.", first),
            (PresenceAction::Joined, false) => crate::rich_text!("📥 **{}** and **{}** others have joined the battle!", first, others.len()),
            (PresenceAction::Left, false) => crate::rich_text!("📤 **{}** and **{}** others have left the battle!", first, others.len()),
        })
    }

    fn flush_presence(&mut self, action: PresenceAction) {
        let pending = match action {
            PresenceAction::Joined => &mut self.pending_joins,
            PresenceAction::Left => &mut self.pending_leaves,
        };
        let content = Self::presence_message(pending, action);
        pending.clear();
        if let Some(message) = content {
            self.broadcaster.publish(BroadcastMessage::from(message));
        }
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
                if self.pending_joins.len() >= MAX_BATCH_SIZE {
                    self.flush_presence(PresenceAction::Joined);
                }
            }
            GameEvent::LeaveEvent(leave) => {
                self.pending_leaves.push(leave.name.clone());
                if self.pending_leaves.len() >= MAX_BATCH_SIZE {
                    self.flush_presence(PresenceAction::Left);
                }
            }
            GameEvent::MapChangeEvent(map_change) => {
                let new_map_name = &map_change.new_map_name;

                if !unsafe { SERVER_STARTED } {
                    self.broadcaster.publish(
                        BroadcastMessage::new()
                            .title("🚀️ Server Started")
                            .content(format!("Map {new_map_name}."))
                            .color(0x5865F2)
                    );
                    unsafe { SERVER_STARTED = true };
                } else {
                    self.broadcaster.publish(
                        BroadcastMessage::new()
                            .title("🗺️ Map Change")
                            .content(format!("Starting match on {new_map_name}."))
                            .color(0x5865F2)
                    );
                }
            },
            GameEvent::MatchEndEvent(match_end) => {
                self.broadcaster.publish(
                    BroadcastMessage::new()
                        .title("🏁 Match Ended")
                        .field("Winner", match_end.winner_team.as_str())
                        .field("Score", match_end.final_score.as_str())
                        .color(0x5865F2)
                );
            },
            GameEvent::ServerRestartEvent => {
                self.broadcaster.publish(
                    BroadcastMessage::new()
                        .title("🔄 Server Restart")
                        .content("Server is restarting")
                        .color(0xE67E22)
                );
            }
            _ => {}
        }
    }

    async fn on_tick(&mut self) {
        self.flush_presence(PresenceAction::Joined);
        self.flush_presence(PresenceAction::Left);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case::test_case(1, PresenceAction::Joined, "📥 **Alice** joined the fray."; "single_join")]
    #[test_case::test_case(1, PresenceAction::Left, "📤 **Alice** left the battle."; "single_leave")]
    #[test_case::test_case(2, PresenceAction::Joined, "📥 **Alice** and **1** others have joined the battle!"; "batch_join")]
    #[test_case::test_case(2, PresenceAction::Left, "📤 **Alice** and **1** others have left the battle!"; "batch_leave")]
    fn presence_templates_preserve_text_and_emphasis(count: usize, action: PresenceAction, expected: &str) {
        let players = vec!["Alice".into(), "Bob".into()];
        let text = EventBroadcastSubscriber::presence_message(&players[..count], action).unwrap();
        assert_eq!(text.to_discord(), expected);
    }

    #[tokio::test]
    async fn tick_flushes_joins_then_leaves_without_repeating_them() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let broadcaster = Box::leak(Box::new(EventPublisher::new(sender)));
        let mut subscriber = EventBroadcastSubscriber::new(broadcaster);
        subscriber.pending_joins.push("Alice".into());
        subscriber.pending_leaves.push("Bob".into());
        subscriber.on_tick().await;
        for expected in ["📥 **Alice** joined the fray.", "📤 **Bob** left the battle."] {
            let message = receiver.try_recv().unwrap();
            assert_eq!(message.content_ref().unwrap().to_discord(), expected);
        }
        assert!(subscriber.pending_joins.is_empty());
        assert!(subscriber.pending_leaves.is_empty());
        subscriber.on_tick().await;
        assert!(receiver.try_recv().is_err());
    }
}
