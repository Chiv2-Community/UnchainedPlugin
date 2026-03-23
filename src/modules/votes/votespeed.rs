use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::modules::vote::{VoteType, ErasedVoteType, VoteTypeHandler};
use clap::Parser;

#[derive(Parser, Clone, Debug)]
#[command(name = "speed")]
pub struct SpeedVoteArgs {
    pub speed: u32
}

#[derive(Clone)]
pub struct SpeedVote;

impl VoteType<SpeedVoteArgs> for SpeedVote {
    fn title(&self) -> String { "Game Speed".into() }
    fn description(&self) -> String { "Vote to change the global game speed. 100 is default speed, 200 is 2x, etc;".into() }
    fn vote_description(&self, args: SpeedVoteArgs) -> String {
        format!("Vote to set game speed to {:.2}%", args.speed)
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.6 }
    fn min_votes_required_ratio(&self) -> f32 { 0.5 }
    fn on_success(&self, args: SpeedVoteArgs) {
        let mut queue = NATIVE_COMMAND_QUEUE.lock().unwrap();
        queue.push(format!("slomo {:.2}", (args.speed as f64) / 100f64));
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}
