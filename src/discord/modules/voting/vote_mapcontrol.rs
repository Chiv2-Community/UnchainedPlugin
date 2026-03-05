use crate::discord::{modules::voting::vote_module::VoteType, events::GameCommand};

#[derive(Clone)]
pub struct RestartVote;

#[async_trait::async_trait]
impl VoteType for RestartVote {
    fn title(&self) -> String { "Restart Round".into() }
    fn description(&self) -> String { "Vote to restart the current round immediately.".into() }
    fn min_ratio(&self) -> f32 { 0.5 }
    fn min_votes(&self) -> usize { 3 }

    fn check_prerequisites(&self, _cmd: &GameCommand) -> Result<(), String> {
        Ok(())
    }

    async fn on_success(&self, _target: &str) {
        // TODO: implement
        // Get current map name
        // servertravel
        // crate::commands::NATIVE_COMMAND_QUEUE.lock().unwrap().push("...".into());
    }
}

#[derive(Clone)]
pub struct EndMapVote;

#[async_trait::async_trait]
impl VoteType for EndMapVote {
    fn title(&self) -> String { "End Match".into() }
    fn description(&self) -> String { "Vote to end the current match immediately.".into() }
    fn min_ratio(&self) -> f32 { 0.6 }
    fn min_votes(&self) -> usize { 3 }

    fn check_prerequisites(&self, _cmd: &GameCommand) -> Result<(), String> {
        Ok(())
    }

    async fn on_success(&self, _target: &str) {
        let mut queue = crate::commands::NATIVE_COMMAND_QUEUE.lock().unwrap();
        queue.push("tbsendgame 1".into());
    }
}