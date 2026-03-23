use crate::modules::vote::{VoteType, ErasedVoteType, VoteTypeHandler};
use clap::Parser;

#[derive(Parser, Clone, Debug)]
#[command(name = "kick")]
pub struct KickVoteArgs {
    pub player_name: String,
}

#[derive(Clone)]
pub struct KickVote;

// Delete this? Vote kicks are built in to the game
impl VoteType<KickVoteArgs> for KickVote {
    fn title(&self) -> String { "kick".into() }
    fn description(&self) -> String { "Vote to kick a player from the server.".into() }
    fn vote_description(&self, args: KickVoteArgs) -> String {
        format!("Vote to kick {}", args.player_name)
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.6 }
    fn min_votes_required_ratio(&self) -> f32 { 0.1 }
    fn on_success(&self, args: KickVoteArgs) {
        println!("Kicking player: {}", args.player_name);
        // Real implementation would call game API to kick
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}
