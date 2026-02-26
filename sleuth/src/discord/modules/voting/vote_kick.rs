use crate::discord::{modules::voting::vote_module::VoteType, notifications::{CommandSource, GameCommandEvent}};

pub struct KickVote;

#[async_trait::async_trait]
impl VoteType for KickVote {
    fn title(&self) -> String { 
        "Kick Player".into() 
    }

    fn description(&self) -> String { 
        "Vote to remove a disruptive player from the server. Please ensure there is a valid reason before voting.".into() 
    }

    /// Requires a 2/3 majority (66%) to pass
    fn min_ratio(&self) -> f32 { 
        0.66 
    }

    /// Requires at least 4 people to participate to be valid
    fn min_votes(&self) -> usize { 
        4 
    }

    fn check_prerequisites(&self, cmd: &GameCommandEvent) -> Result<(), String> {
        if cmd.source != CommandSource::GameChat {
            return Err("Kick votes can only be started from in-game chat.".into());
        }
        let [_player_name, ..] = cmd.args.as_slice() else {
            return Err("You must specify a player name to kick.".into());
        };
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn VoteType> {
        Box::new(KickVote) 
    }
    
    async fn on_success(&self, target: &str) {
        // TODO: implement
        // Find player by name?
        println!("[VoteSystem] Kick vote passed for target: {}", target);
    }
}