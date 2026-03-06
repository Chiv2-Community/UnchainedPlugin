use serenity::all::{ChannelId, Http};

use crate::discord::core::DiscordSubscriber;
use crate::discord::events::GameEvent;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use crate::discord::responses::*;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct GlobalStats {
    total_kills: HashMap<String, u32>,
}

pub struct StatsTracker {
    session_kills: HashMap<String, u32>, // Resets every match
    global_stats: GlobalStats,           // Persistent
    storage_path: String,
}

impl StatsTracker {
    pub fn new(storage_path: &str) -> Self {
        // Load existing global stats or start fresh
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

        // 1. Extract into a Vec and Sort
        let mut entries: Vec<(&String, &u32)> = self.global_stats.total_kills.iter().collect();
        
        // Sort descending by kill count
        entries.sort_by(|a, b| b.1.cmp(a.1));

        // 2. Format the output
        let mut message = String::from("🏆 **All-Time Top 5 Killers** 🏆\n```rust\n");
        message.push_str(&format!("{:<4} {:<20} {:<10}\n", "#", "Player", "Kills"));
        message.push_str(&"-".repeat(36));
        message.push('\n');

        for (i, (name, kills)) in entries.iter().take(5).enumerate() {
            // Truncate long names to keep the table clean
            let display_name = if name.len() > 18 { &name[..18] } else { name };
            message.push_str(&format!("{:<4} {:<20} {:<10}\n", i + 1, display_name, kills));
        }

        message.push_str("```");
        message
    }
}

#[async_trait::async_trait]
impl DiscordSubscriber for StatsTracker {
    fn name(&self) -> &'static str { "StatsTracker" }

    async fn on_event(&mut self, event: &GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        match event {
            // 1. Track Kills
            GameEvent::KillEvent(kill) => {
                let killer_name = &kill.killer_name;
                *self.session_kills.entry(killer_name.clone()).or_insert(0) += 1;
                *self.global_stats.total_kills.entry(killer_name.clone()).or_insert(0) += 1;
                NO_RESP
            }

            // 2. Handle Match End (Report & Reset)
            GameEvent::MatchEndEvent(_end) => {
                // Find Top Performer of the round
                let mvp = self.session_kills.iter()
                    .max_by_key(|entry| entry.1);

                let mut report = String::from("🏰 **Match Concluded!**\n");
                if let Some((name, kills)) = mvp {
                    report.push_str(&format!("🏆 **MVP:** {} with {} kills!\n", name, kills));
                }

                // Save Lifetime stats to JSON
                self.save_to_disk();
                
                // Clear Session for next round
                self.session_kills.clear();

                msg(report).into_responses()
            }
            
            GameEvent::CommandRequestEvent(cmd) => {
                match cmd.command.as_str() {
                    "!top" | "!leaderboard" => {
                        msg(self.get_top_5_leaderboard()).into_responses()
                    },
                    "!mystats" => {
                        let total = self.global_stats.total_kills.get(&cmd.user).unwrap_or(&0);
                        msg(
                            format!("📊 **{}**, you have **{}** total kills.", cmd.user, total)
                        ).into_responses()
                    },
                    _ => NO_RESP,
                }
            }

            _ => NO_RESP,
        }
    }
}