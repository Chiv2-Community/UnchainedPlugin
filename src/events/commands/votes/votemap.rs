use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::events::commands::vote::{VoteType, ErasedVoteType, VoteTypeHandler};
use clap::Parser;

#[derive(Parser, Clone, Debug)]
pub struct MapVoteArgs {
    pub map_name: String,
}

#[derive(Clone)]
pub struct MapVote;
impl VoteType<MapVoteArgs> for MapVote {
    fn title(&self) -> String { "Map Change".into() }
    fn description(&self) -> String { "Vote to change the server to a new map.".into() }
    fn vote_description(&self, args: MapVoteArgs) -> String {
        format!("Map: {}", args.map_name)
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.5 }
    fn min_votes_required_ratio(&self) -> f32 { 0.1 }
    fn on_success(&self, args: MapVoteArgs) {
        let mut queue = NATIVE_COMMAND_QUEUE.lock().unwrap();
        queue.push(format!("servertravel {}", args.map_name));
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}