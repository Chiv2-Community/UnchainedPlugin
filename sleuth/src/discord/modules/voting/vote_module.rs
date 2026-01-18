use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use std::any::Any;
use serenity::builder::{CreateMessage, CreateEmbed, CreateEmbedFooter};
use serenity::model::id::ChannelId;
use serenity::http::Http;
use crate::discord::core::{DiscordSubscriber, GameEvent};
use crate::discord::modules::voting::vote_bots::{AddBotsVote, NoBotsVote};
use crate::discord::modules::voting::vote_kick::KickVote;
use crate::discord::modules::voting::vote_map::MapVote;
use crate::discord::modules::voting::vote_mapcontrol::{EndMapVote, RestartVote};
use crate::discord::modules::voting::vote_mod::ModVote;
use crate::discord::notifications::{CommandSource, GameCommandEvent};
use crate::discord::responses::*;

/// Trait that defines a specific type of vote's behavior
#[async_trait::async_trait]
pub trait VoteType: Send + Sync {
    fn title(&self) -> String;
    fn description(&self) -> String;
    fn min_ratio(&self) -> f32;
    fn min_votes(&self) -> usize;
    async fn on_success(&self, target: &str);
    fn clone_box(&self) -> Box<dyn VoteType>;

    fn check_prerequisites(&self, cmd: &GameCommandEvent) -> Result<(), String> {
        if cmd.args.is_empty() {
            return Err("This vote requires a target argument.".into());
        }
        Ok(())
    }
}

pub struct VoteModule {
    active_vote: Option<ActiveVote>,
    registry: HashMap<String, Box<dyn VoteType>>,
    ctx: crate::discord::Ctx,
}

struct ActiveVote {
    logic: Box<dyn VoteType>,
    target: String,
    yes_votes: HashSet<String>,
    no_votes: HashSet<String>,
    end_time: std::time::Instant,
    last_broadcast: std::time::Instant,
}

impl VoteModule {
    // Matches your existing initialization pattern
    pub fn new(ctx: crate::discord::Ctx) -> Self {
        let mut registry: HashMap<String, Box<dyn VoteType>> = HashMap::new();

        registry.insert("votemap".into(), Box::new(MapVote));
        registry.insert("voterestart".into(), Box::new(RestartVote));
        registry.insert("voteendmap".into(), Box::new(EndMapVote));
        registry.insert("votemod".into(), Box::new(ModVote));
        registry.insert("votekick".into(), Box::new(KickVote));
        registry.insert("voteaddbots".into(), Box::new(AddBotsVote));
        registry.insert("votenobots".into(), Box::new(NoBotsVote));
        registry.insert("voterestart".into(), Box::new(RestartVote));
        registry.insert("voteendmap".into(), Box::new(EndMapVote));

        Self {
            active_vote: None,
            registry,
            ctx,
        }
    }

    fn get_help_embed(&self) -> CreateMessage {
        let mut embed = CreateEmbed::new()
            .title("🗳️ Voting System Help")
            .description("Use these commands to start a server-wide vote:")
            .color(0xF1C40F); // Gold color

        for (name, logic) in &self.registry {
            embed = embed.field(
                format!("!{}", name),
                format!("_{}_\n(Requires {}% ratio, {} min votes)", 
                    logic.description(), 
                    (logic.min_ratio() * 100.0) as i32, 
                    logic.min_votes()),
                false
            );
        }
        CreateMessage::new().add_embed(embed)
    }

    fn init_vote(&mut self, logic: Box<dyn VoteType>, target: String, initiator: String) -> CreateMessage {
        let now = std::time::Instant::now();
        
        self.active_vote = Some(ActiveVote {
            logic,
            target: target.clone(),
            yes_votes: HashSet::from([initiator.clone()]),
            no_votes: HashSet::new(),
            end_time: now + std::time::Duration::from_secs(20),
            last_broadcast: now,
        });

        let state = self.active_vote.as_ref().unwrap();
        let embed = CreateEmbed::new()
            .title(format!("🗳️ Vote Started: {}", state.logic.title()))
            .description(format!("{}\n\n**Target:** `{}`\n**By:** {}", state.logic.description(), target, initiator))
            .color(0x3498db)
            .footer(CreateEmbedFooter::new("Type !yes or !no in chat"));

        CreateMessage::new().add_embed(embed)
    }

    async fn resolve_vote(&mut self) -> CreateMessage {
        let state = self.active_vote.take().expect("Vote resolution on None");
        let yes = state.yes_votes.len();
        let no = state.no_votes.len();
        let total = yes + no;
        
        let ratio = if total > 0 { yes as f32 / total as f32 } else { 0.0 };
        let passed = ratio >= state.logic.min_ratio() && yes >= state.logic.min_votes();

        let mut embed = CreateEmbed::new()
            .title(format!("Results: {}", state.logic.title()))
            .field("Final Count", format!("✅ {} Yes | ❌ {} No", yes, no), true);

        if passed {
            // Trigger the custom function provided by the specific vote class
            state.logic.on_success(&state.target).await;
            embed = embed.description(format!("✅ **Passed!** Target `{}` executed.", state.target))
                         .color(0x2ecc71);
        } else {
            embed = embed.description(format!("❌ **Failed.** Requirements not met for `{}`.", state.target))
                         .color(0xe74c3c);
        }

        CreateMessage::new().add_embed(embed)
    }
}

#[async_trait::async_trait]
impl DiscordSubscriber for VoteModule {
    fn name(&self) -> &'static str { "VoteModule" }

    async fn on_event(&mut self, event: &dyn GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        let any = event.as_any();
        
        if let Some(cmd) = any.downcast_ref::<GameCommandEvent>() {


            if cmd.name == "votehelp" {
                return BotResponse::from(self.get_help_embed()).into_responses();
            }

            if cmd.source != CommandSource::GameChat { return NO_RESP; }
            
            // Handle voting input
            if let Some(ref mut state) = self.active_vote {
                let voter = cmd.actor.display_name.clone();
                match cmd.name.as_str() {
                    "yes" => { state.no_votes.remove(&voter); state.yes_votes.insert(voter); },
                    "no" => { state.yes_votes.remove(&voter); state.no_votes.insert(voter); },
                    _ => {}
                }
                return NO_RESP;
            }   
                    
            if let Some(logic_prototype) = self.registry.get(&cmd.name) {
                if let Err(err) = logic_prototype.check_prerequisites(cmd) {
                    return msg(format!("⚠️ {}", err)).into_responses();
                }

                let logic = logic_prototype.clone_box();
                
                let target = cmd.args.first().cloned().unwrap_or_else(|| "Server".to_string());
                let initiator = cmd.actor.display_name.clone();

                return BotResponse::from(self.init_vote(logic, target, initiator)).into_responses();
            }
        }
        NO_RESP
    }

    async fn on_tick(&mut self, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        let now = std::time::Instant::now();
        
        if let Some(ref mut state) = self.active_vote {
            if now >= state.end_time {
                return BotResponse::from(self.resolve_vote().await).into_responses();
            }

            if now.duration_since(state.last_broadcast).as_secs() >= 5 {
                state.last_broadcast = now;
                let remaining = state.end_time.duration_since(now).as_secs();
                return msg(format!("⏳ **Vote Active:** {} Yes | {} No ({}s remaining)", 
                    state.yes_votes.len(), state.no_votes.len(), remaining)).into_responses();
            }
        }
        NO_RESP
    }
}