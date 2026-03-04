use std::any::Any;
use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::discord::core::DiscordSubscriber;
use crate::discord::notifications::GameEvent;
use crate::discord::notifications::{CommandSource, GameChatMessage, GameCommandEvent};
use crate::discord::responses::*;

#[derive(Debug, Clone)]
pub struct MapVoteEvent {
    pub initiator: String,
    pub map_target: String,
}

use std::sync::Arc;
use std::collections::HashSet;
use serenity::builder::CreateMessage;
use serenity::model::id::ChannelId;
use serenity::http::Http;

pub struct ExtMapVote {
    active_vote: Option<VoteState>,
    ctx: crate::discord::Ctx
}


struct VoteState {
    map_name: String,
    yes_votes: HashSet<String>,
    no_votes: HashSet<String>,
    start_time: std::time::Instant,
    last_broadcast: std::time::Instant,
    end_time: std::time::Instant,
}

#[async_trait::async_trait]
impl DiscordSubscriber for ExtMapVote {
    fn name(&self) -> &'static str { "ExtMapVote" }

    async fn on_event(&mut self, event: &GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        match event {
            GameEvent::GameCommand(cmd) => {
                if cmd.source != CommandSource::GameChat { return NO_RESP; }

                match self.active_vote {
                    Some(ref mut state) => {
                        let actor_name = cmd.actor.display_name.clone();
                        if cmd.name == "yes" {
                            state.no_votes.remove(&actor_name);
                            state.yes_votes.insert(actor_name.clone());
                        }
                        if cmd.name == "no" {
                            state.yes_votes.remove(&actor_name);
                            state.no_votes.insert(actor_name.clone());
                        }
                        NO_RESP
                    }
                    None => {
                        if cmd.name != "startvotemap" { return NO_RESP; }
                        let [map_name, ..] = cmd.args.as_slice() else { return NO_RESP; };
                        let actor_name = cmd.actor.display_name.clone();
                        let map_name = map_name.clone();
                        if let Some(msg) = self.init_vote(actor_name, map_name) {
                            BotResponse::from(msg).into_responses()
                        } else {
                            NO_RESP
                        }
                    }
                }
            }
            _ => NO_RESP
        }
    }

    async fn on_tick(&mut self, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        let now = std::time::Instant::now();
        // crate::sinfo!(f; "Tick");
        
        // Use a mutable reference to update last_broadcast
        if let Some(ref mut state) = self.active_vote {
            // crate::sinfo!(f; "Vote Tick");
            // 1. Check for Final Timeout
            if now >= state.end_time {
                if let Some(msg) = self.resolve_vote() {
                    return BotResponse::from(msg).into_responses();
                }
                return NO_RESP;
            }

            // 2. Check for 5-second Interval Broadcast
            // We ensure we don't broadcast if the vote is basically over (e.g., within 1s of end)
            if now.duration_since(state.last_broadcast).as_secs() >= 5 && (state.end_time - now).as_secs() > 1 {
                state.last_broadcast = now;
                let remaining = state.end_time.duration_since(now).as_secs();
                
                return msg(format!(
                    "⏳ **Vote Progress** (`{}`): {} Yes | {} No ({}s remaining)",
                    state.map_name,
                    state.yes_votes.len(),
                    state.no_votes.len(),
                    remaining
                )).into_responses();
            }
        }
        NO_RESP
    }
}

impl ExtMapVote {
    pub fn new(ctx: crate::discord::Ctx) -> Self {
        Self {
            active_vote: None,
            ctx
        }
    }
    
    fn init_vote(&mut self, initiator: String, map: String) -> Option<CreateMessage> {
        if self.active_vote.is_some() {
            return Some(CreateMessage::new().content("⚠️ A vote is already active."));
        }

        let now = std::time::Instant::now();
        self.active_vote = Some(VoteState {
            map_name: map.clone(),
            yes_votes: {
                let mut votes = HashSet::new();
                votes.insert(initiator.clone());
                votes
            },
            no_votes: HashSet::new(),
            start_time: now,
            last_broadcast: now,
            end_time: now + std::time::Duration::from_secs(15),
        });

        Some(CreateMessage::new().content(format!(
            "🗳️ **{}** wants to change map to `{}`!\nType `!yes` or `!no` in chat now!", 
            initiator, map
        )))
    }

    fn resolve_vote(&mut self) -> Option<CreateMessage> {
        if let Some(state) = self.active_vote.take() {
            let yes = state.yes_votes.len();
            let no = state.no_votes.len();
            
            if yes > no && yes > 0 {
                let map_clone = state.map_name.clone();
                NATIVE_COMMAND_QUEUE.lock().unwrap_or_else(|e| e.into_inner()).push(format!("servertravel {map_clone}"));
                return Some(CreateMessage::new().content(format!("✅ **Vote Passed!** {} to {}. Traveling...", yes, no)));
            }
            return Some(CreateMessage::new().content(format!("❌ **Vote Failed.** Final score: {} Yes, {} No.", yes, no)));
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct VoteCastEvent {
    pub voter_id: String,
    pub choice: bool, // true = Yes, false = No
}