use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::events::integrations::game_event::vote::{VoteType, ErasedVoteType, VoteTypeHandler};
use clap::Parser;

#[derive(Parser, Clone, Debug)]
#[command(name = "add-bots")]
pub struct AddBotsVoteArgs {
    pub count: i32,
}

#[derive(Clone)]
pub struct AddBotsVote;
impl VoteType<AddBotsVoteArgs> for AddBotsVote {
    fn title(&self) -> String { "Add Bots".into() }
    fn description(&self) -> String { "Vote to add AI bots to the current game.".into() }
    fn vote_description(&self, args: AddBotsVoteArgs) -> String {
        format!("Vote to add {} AI bots", args.count)
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.5 }
    fn min_votes_required_ratio(&self) -> f32 { 0.1 }
    fn on_success(&self, args: AddBotsVoteArgs) {
        let mut queue = NATIVE_COMMAND_QUEUE.lock().unwrap();
        queue.push(format!("addplayerbots {}", args.count));
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}

#[derive(Parser, Clone, Debug)]
#[command(name = "no-bots")]
pub struct NoBotsVoteArgs {}

#[derive(Clone)]
pub struct NoBotsVote;
impl VoteType<NoBotsVoteArgs> for NoBotsVote {
    fn title(&self) -> String { "Remove Bots".into() }
    fn description(&self) -> String { "Vote to remove all AI bots from the server.".into() }
    fn vote_description(&self, _args: NoBotsVoteArgs) -> String {
        "Vote to remove all AI bots".to_string()
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.5 }
    fn min_votes_required_ratio(&self) -> f32 { 0.1 }
    fn on_success(&self, _args: NoBotsVoteArgs) {
        let mut queue = NATIVE_COMMAND_QUEUE.lock().unwrap();
        queue.push("disablespawningbots".into());
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}
