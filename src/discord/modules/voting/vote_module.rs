use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use serenity::all::{ChannelId, CreateEmbed, CreateEmbedFooter, CreateMessage, Http};
use sleuth_macros::handler_command;
use crate::discord::core::DiscordSubscriber;
use crate::discord::events::GameEvent;
use crate::discord::modules::voting::vote_bots::{AddBotsVote, NoBotsVote};
use crate::discord::modules::voting::vote_kick::KickVote;
use crate::discord::modules::voting::vote_map::MapVote;
use crate::discord::modules::voting::vote_mapcontrol::{EndMapVote, RestartVote};
use crate::discord::modules::voting::vote_mod::ModVote;
use crate::discord::events::GameCommand;
use crate::discord::responses::*;


/// Trait that defines a specific type of vote's behavior
pub trait BoxClone {
    fn clone_box(&self) -> Box<dyn VoteType>;
}

impl<T> BoxClone for T
where
    T: 'static + VoteType + Clone,
{
    fn clone_box(&self) -> Box<dyn VoteType> {
        Box::new(self.clone())
    }
}

#[async_trait::async_trait]
pub trait VoteType: Send + Sync + BoxClone {
    fn title(&self) -> String;
    fn description(&self) -> String;
    fn min_ratio(&self) -> f32;
    fn min_votes(&self) -> usize;

    fn check_prerequisites(&self, cmd: &GameCommand) -> Result<(), String> {
        let [_target, ..] = cmd.args.as_slice() else {
            return Err("This vote requires a target argument.".into());
        };
        Ok(())
    }
    
    async fn on_success(&self, target: &str);


}

pub struct VoteModule {
    active_vote: Option<ActiveVote>,
    registry: HashMap<String, Box<dyn VoteType>>,

    #[allow(dead_code)]
    _ctx: crate::discord::Ctx,
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
    

    // TODO: A single user should not be able to enact multiple consecutive votes rapidly.  A malicious user could spam a map vote or vote kick to avoid a vote kick being made against them.
    fn run_registry_vote(&mut self, vote_type: &str, target: String, cmd: &GameCommand) -> Vec<BotResponse> {
        if let Some(logic_prototype) = self.registry.get(vote_type) {
            // Check prerequisites using the prototype
            if let Err(err) = logic_prototype.check_prerequisites(cmd) {
                return msg(format!("⚠️ {}", err)).into_responses();
            }

            let logic = logic_prototype.clone_box();
            let initiator = cmd.actor.display_name.clone();

            return BotResponse::from(self.init_vote(logic, target, initiator)).into_responses();
        }
        msg(format!("❌ Unknown vote type: {}", vote_type)).into_responses()
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
        
        let state = ActiveVote {
            logic,
            target: target.clone(),
            yes_votes: HashSet::from([initiator.clone()]),
            no_votes: HashSet::new(),
            end_time: now + std::time::Duration::from_secs(20),
            last_broadcast: now,
        };

        let embed = CreateEmbed::new()
            .title(format!("🗳️ Vote Started: {}", state.logic.title()))
            .description(format!("{}\n\n**Target:** `{}`\n**By:** {}", state.logic.description(), target, initiator))
            .color(0x3498db)
            .footer(CreateEmbedFooter::new("Type !yes or !no in chat"));

        self.active_vote = Some(state);

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

    // Command handlers

    #[handler_command("votehelp")]
    pub fn cmd_help(&mut self) -> Vec<BotResponse> {

        BotResponse::from(self.get_help_embed()).into_responses()
    }

    #[handler_command("cancelvote", desc = "Vote YES on the active poll", elevated = true)]
    pub fn cmd_cancelvote(&mut self, _cmd: &GameCommand) -> Vec<BotResponse> {
        if let Some(ref mut state) = self.active_vote {
            let resp = BotResponse::from(CreateEmbed::new()
            .title(format!("🗳️ Vote Cancelled by Admin: {}", state.logic.title()))
            .description(format!("{}", state.logic.description()))
            .color(0x3498db)).into_responses();
            self.active_vote = None;
            return resp;
        }
        msg("No active Vote found").into_responses()
    }

    #[handler_command("yes", desc = "Vote YES on the active poll", source = "GameChat")]
    pub fn cmd_yes(&mut self, cmd: &GameCommand) -> Vec<BotResponse> {
        if let Some(ref mut state) = self.active_vote {
            let voter = cmd.actor.display_name.clone();
            state.no_votes.remove(&voter);
            state.yes_votes.insert(voter);
        }
        NO_RESP
    }

    #[handler_command("no", desc = "Vote YES on the active poll", source = "GameChat")]
    pub fn cmd_no(&mut self, cmd: &GameCommand) -> Vec<BotResponse> {
        if let Some(ref mut state) = self.active_vote {
            let voter = cmd.actor.display_name.clone();
            state.yes_votes.remove(&voter);
            state.no_votes.insert(voter);
        }
        NO_RESP
    }

    #[handler_command(name = "votekick", desc = "Vote to remove a disruptive player. Ensure there is a valid reason.")]
    pub fn cmd_votekick(&mut self, player_name: String, cmd: &GameCommand) -> Vec<BotResponse> {
        self.run_registry_vote("votekick", player_name, cmd)
    }

    #[handler_command(name = "votemap", desc = "Vote to change the server to a new map.")]
    pub fn cmd_votemap(&mut self, map_name: String, cmd: &GameCommand) -> Vec<BotResponse> {
        self.run_registry_vote("votemap", map_name, cmd)
    }

    #[handler_command(name = "voterestart", desc = "Vote to restart the current round immediately.")]
    pub fn cmd_voterestart(&mut self, cmd: &GameCommand) -> Vec<BotResponse> {
        self.run_registry_vote("voterestart", "Current Round".to_string(), cmd)
    }

    #[handler_command(name = "voteendmap", desc = "Vote to end the current map.")]
    pub fn cmd_voteendmap(&mut self, cmd: &GameCommand) -> Vec<BotResponse> {
        self.run_registry_vote("voteendmap", "Current Round".to_string(), cmd)
    }

    #[handler_command(name = "voteaddbots", desc = "Vote to add AI bots to the current game.")]
    pub fn cmd_voteaddbots(&mut self, count: String, cmd: &GameCommand) -> Vec<BotResponse> {
        self.run_registry_vote("voteaddbots", count, cmd)
    }

    #[handler_command(name = "votemod", desc = "Enable a mod.")]
    pub fn cmd_votemod(&mut self, _mod_name: String, cmd: &GameCommand) -> Vec<BotResponse> {
        self.run_registry_vote("votemod", "Current Round".to_string(), cmd)
    }

    #[handler_command(name = "votenobots", desc = "Vote to add AI bots to the current game.")]
    pub fn cmd_votenobots(&mut self, cmd: &GameCommand) -> Vec<BotResponse> {
        self.run_registry_vote("votenobots", "Current Round".to_string(), cmd)
    }

    pub fn new(ctx: crate::discord::Ctx) -> Self {
        let mut registry: HashMap<String, Box<dyn VoteType>> = HashMap::new();

        // TODO: Clean this up. There is a tight and unchecked coupling between the record registry, the handler_command invocations above, and the run_registry_vote invocations. There is likely an effective way to move most of this in to an interface and define these relationships once.
        registry.insert("votekick".into(), Box::new(KickVote));
        registry.insert("votemap".into(), Box::new(MapVote));
        registry.insert("voterestart".into(), Box::new(RestartVote));
        registry.insert("voteendmap".into(), Box::new(EndMapVote));
        registry.insert("votemod".into(), Box::new(ModVote));
        registry.insert("voteaddbots".into(), Box::new(AddBotsVote));
        registry.insert("votenobots".into(), Box::new(NoBotsVote));

        Self {
            active_vote: None,
            registry,
            _ctx: ctx,
        }
    }
}

#[async_trait::async_trait]
impl DiscordSubscriber for VoteModule {
    fn name(&self) -> &'static str { "VoteModule" }

    fn get_commands(&self) -> Vec<CommandInfo> {
        crate::auto_help!(self, [
            cmd_help, 
            cmd_cancelvote,
            cmd_yes, 
            cmd_no, 
            cmd_votekick, 
            cmd_votemap, 
            cmd_voterestart,
            cmd_voteendmap,
            cmd_votemod,
            cmd_voteaddbots,
            cmd_votenobots
        ])
    }

    async fn on_event(&mut self, event: &GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        match event {
            GameEvent::GameCommandEvent(cmd) => {
                crate::auto_dispatch!(self, cmd, [
                    cmd_help, 
                    cmd_cancelvote,
                    cmd_yes, 
                    cmd_no, 
                    cmd_votekick, 
                    cmd_votemap, 
                    cmd_voterestart,
                    cmd_voteendmap,
                    cmd_votemod,
                    cmd_voteaddbots,
                    cmd_votenobots
                ]);
                NO_RESP
            }
            _ => NO_RESP,
        }
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