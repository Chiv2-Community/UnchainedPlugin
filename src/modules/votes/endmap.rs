use clap::Parser;
use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::modules::vote::{ErasedVoteType, VoteType, VoteTypeHandler};
use crate::modules::votes::votemap::{MapVote, MapVoteArgs};

#[derive(Parser, Clone, Debug)]
#[command(name = "next-map")]
pub struct EndMapVoteArgs;

#[derive(Clone)]
pub struct EndMapVote;

impl VoteType<EndMapVoteArgs> for EndMapVote {
    fn title(&self) -> String { "End Match".into() }
    fn description(&self) -> String { "Vote to end the current match immediately.".into() }
    fn vote_description(&self, _args: EndMapVoteArgs) -> String {
        self.description()
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.6 }
    fn min_votes_required_ratio(&self) -> f32 { 0.5 }
    fn on_success(&self, _args: EndMapVoteArgs) {
        let mut queue = NATIVE_COMMAND_QUEUE.lock().unwrap();
        queue.push("tbsendgame 1".into());
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}
