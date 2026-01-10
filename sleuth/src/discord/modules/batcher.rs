use crate::discord::responses::{BotResponse, IntoResponses, msg};
use crate::discord::{core::*, responses::NO_RESP};
use crate::discord::notifications::JoinEvent;
use serenity::all::{Http, ChannelId, CreateMessage, CreateEmbed};
use std::sync::Arc;

pub struct JoinBatcher {
    pending_joins: Vec<String>,
    max_batch_size: usize,
}

impl JoinBatcher {
    pub fn default() -> Self {
        Self {
            pending_joins: Vec::new(),
            max_batch_size: 10,
        }
    }
}

#[async_trait::async_trait]
impl DiscordSubscriber for JoinBatcher {
    fn name(&self) -> &'static str { "JoinBatcher" }

    async fn on_event(&mut self, event: &dyn GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        // We only care about JoinEvents
        if let Some(join) = event.as_any().downcast_ref::<JoinEvent>() {
            self.pending_joins.push(join.name.clone());

            // If we hit a massive wave (e.g., 10 people), flush immediately
            if self.pending_joins.len() >= self.max_batch_size {
                return self.flush();
            }
        }
        NO_RESP
    }

    async fn on_tick(&mut self, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        // Every tick (e.g., 1 second), if we have people waiting, send the update
        if !self.pending_joins.is_empty() {
            return self.flush();
        }
        NO_RESP
    }
}

impl JoinBatcher {
    fn flush(&mut self) -> Vec<BotResponse> {
        if self.pending_joins.is_empty() {
            return NO_RESP;
        }

        let content = if self.pending_joins.len() == 1 {
            format!("📥 **{}** joined the fray.", self.pending_joins[0])
        } else {
            format!(
                "📥 **{}** and **{}** others have joined the battle!",
                self.pending_joins[0],
                self.pending_joins.len() - 1
            )
        };

        self.pending_joins.clear();
        
        // Return a clean embed or simple text
        // Some(CreateMessage::new().content(content))
        msg(content).to_main().into_responses()
    }
}