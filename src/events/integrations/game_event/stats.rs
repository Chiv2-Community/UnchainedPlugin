use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;
use clap::Parser;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::{EventPublisher, Subscriber};
use crate::events::integrations::command::Command;
use crate::events::models::{GameCommand, GameEvent, PermissionFlags};

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct GlobalStats {
    total_kills: HashMap<String, u32>,
}

pub struct StatsTrackerState {
    session_kills: HashMap<String, u32>,
    global_stats: GlobalStats,
    storage_path: String,
}

impl StatsTrackerState {
    pub fn new(storage_path: &str) -> Self {
        let global_stats = fs::read_to_string(storage_path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();

        Self {
            session_kills: HashMap::new(),
            global_stats,
            storage_path: storage_path.to_string(),
        }
    }

    fn save_to_disk(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.global_stats) {
            let _ = fs::write(&self.storage_path, json);
        }
    }

    fn get_top_5_leaderboard(&self) -> String {
        if self.global_stats.total_kills.is_empty() {
            return "The history books are empty. No kills recorded yet!".to_string();
        }

        let mut entries: Vec<(&String, &u32)> = self.global_stats.total_kills.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1));

        let mut message = String::from("🏆 **All-Time Top 5 Killers** 🏆\n```rust\n");
        message.push_str(&format!("{:<4} {:<20} {:<10}\n", "#", "Player", "Kills"));
        message.push_str(&"-".repeat(36));
        message.push('\n');

        for (i, (name, kills)) in entries.iter().take(5).enumerate() {
            let display_name = if name.len() > 18 { &name[..18] } else { name };
            message.push_str(&format!("{:<4} {:<20} {:<10}\n", i + 1, display_name, kills));
        }

        message.push_str("```");
        message
    }
}

pub struct StatsTrackerSubscriber {
    state: Arc<Mutex<StatsTrackerState>>,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl StatsTrackerSubscriber {
    pub fn new(state: Arc<Mutex<StatsTrackerState>>, broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self { state, broadcaster }
    }
}

#[async_trait]
impl Subscriber<GameEvent> for StatsTrackerSubscriber {
    fn identifier(&self) -> &'static str {
        "StatsTrackerSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        let mut state = self.state.lock().await;
        
        match event {
            GameEvent::KillEvent(kill) => {
                *state.session_kills.entry(kill.killer.clone()).or_insert(0) += 1;
                *state.global_stats.total_kills.entry(kill.killer.clone()).or_insert(0) += 1;
            }
            GameEvent::MatchEndEvent(_) => {
                let mvp = state.session_kills.iter().max_by_key(|entry| entry.1);

                let mut report = String::from("🏰 **Match Concluded!**\n");
                if let Some((name, kills)) = mvp {
                    report.push_str(&format!("🏆 **MVP:** {} with {} kills!\n", name, kills));
                }

                state.save_to_disk();
                state.session_kills.clear();

                self.broadcaster.publish(BroadcastMessage::from(report));
            }
            _ => {}
        }
    }

    async fn on_tick(&mut self) {}
}

#[derive(Parser, Clone)]
#[command(name = "top", about = "Show the top 5 killers")]
pub struct TopArgs {}

pub struct TopCommand {
    state: Arc<Mutex<StatsTrackerState>>,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl TopCommand {
    pub fn new(state: Arc<Mutex<StatsTrackerState>>, broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self { state, broadcaster }
    }
}

#[async_trait]
impl Command<TopArgs> for TopCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::USER
    }

    async fn execute(&self, _args: TopArgs, _command: &GameCommand) {
        let state = self.state.lock().await;
        self.broadcaster.publish(BroadcastMessage::from(state.get_top_5_leaderboard()));
    }
}

#[derive(Parser, Clone)]
#[command(name = "mystats", about = "Show your total kills")]
pub struct MyStatsArgs {}

pub struct MyStatsCommand {
    state: Arc<Mutex<StatsTrackerState>>,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl MyStatsCommand {
    pub fn new(state: Arc<Mutex<StatsTrackerState>>, broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self { state, broadcaster }
    }
}

#[async_trait]
impl Command<MyStatsArgs> for MyStatsCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::USER
    }

    async fn execute(&self, _args: MyStatsArgs, command: &GameCommand) {
        let state = self.state.lock().await;
        let total = state.global_stats.total_kills.get(&command.actor.display_name).unwrap_or(&0);
        self.broadcaster.publish(BroadcastMessage::from(format!(
            "📊 **{}**, you have **{}** total kills.",
            command.actor.display_name, total
        )));
    }
}
