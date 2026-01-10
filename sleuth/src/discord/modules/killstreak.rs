use crate::discord::core::*;
use crate::discord::notifications::KillEvent;
use serenity::all::{Http, ChannelId, CreateMessage, CreateEmbed};
use std::collections::HashMap;
use std::sync::Arc;
use crate::discord::responses::*;

pub struct KillstreakModule {
    // Tracks Name -> Current Kill Count
    streaks: HashMap<String, u32>,
}

impl KillstreakModule {
    pub fn new() -> Self {
        Self {
            streaks: HashMap::new(),
        }
    }
}

#[async_trait::async_trait]
impl DiscordSubscriber for KillstreakModule {
    fn name(&self) -> &'static str { "KillstreakModule" }

    async fn on_event(&mut self, event: &dyn GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        // Look for KillEvents
        if let Some(kill) = event.as_any().downcast_ref::<KillEvent>() {
            // 1. Reset the victim's streak
            self.streaks.remove(&kill.victim);

            // 2. Increment the killer's streak
            let entry = self.streaks.entry(kill.killer.clone()).or_insert(0);
            *entry += 1;
            let current_streak = *entry;

            // 3. Only return a message on milestones
            return match current_streak {
                5 => BotResponse::from(self.build_embed(&kill.killer, "is on a Killing Spree!", 0x3498db)).into_responses(),
                10 => BotResponse::from(self.build_embed(&kill.killer, "is UNSTOPPABLE!", 0x9b59b6)).into_responses(),
                15 => BotResponse::from(self.build_embed(&kill.killer, "is GODLIKE!", 0xe74c3c)).into_responses(),
                20 => BotResponse::from(self.build_embed(&kill.killer, "is a LEGENDARY WARRIOR!", 0xf1c40f)).into_responses(),
                _ => NO_RESP,
            };
        }
        
        // Note: You could also listen for a "MatchEndEvent" here to clear all streaks
        NO_RESP
    }
}

impl KillstreakModule {
    fn build_embed(&self, player: &str, message: &str, color: u32) -> CreateMessage {
        let embed = CreateEmbed::new()
            .title("⚔️ Killstreak")
            .description(format!("**{}** {}", player, message))
            .color(color);
        CreateMessage::new().add_embed(embed)
    }
}