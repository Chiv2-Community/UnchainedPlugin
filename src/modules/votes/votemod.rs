use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::modules::vote::{VoteType, ErasedVoteType, VoteTypeHandler};
use clap::Parser;

#[derive(Parser, Clone, Debug)]
#[command(name = "mod")]
pub struct ModVoteArgs {
    pub mod_name: String,
    pub enable: bool,
}

#[derive(Clone)]
pub struct ModVote;

impl VoteType<ModVoteArgs> for ModVote {
    fn title(&self) -> String { "Game Modifiers".into() }
    fn description(&self) -> String { "Vote to enable or disable game modifiers.".into() }
    fn vote_description(&self, args: ModVoteArgs) -> String {
        let action = if args.enable { "Enable" } else { "Disable" };
        format!("Vote to {} modifier: {}", action, args.mod_name)
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.6 }
    fn min_votes_required_ratio(&self) -> f32 { 0.5 }
    fn on_success(&self, args: ModVoteArgs) {
        let mut queue = NATIVE_COMMAND_QUEUE.lock().unwrap();
        let cmd = if args.enable { "enablemod" } else { "disablemod" };
        queue.push(format!("{} {}", cmd, args.mod_name));
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}
