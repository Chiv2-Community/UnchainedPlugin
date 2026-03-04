use crate::discord::{modules::voting::vote_module::VoteType, events::GameCommand};

#[derive(Clone)]
pub struct AddBotsVote;

#[async_trait::async_trait]
impl VoteType for AddBotsVote {
    fn title(&self) -> String { "Add Bots".into() }
    fn description(&self) -> String { "Vote to add AI bots to the current game.".into() }
    fn min_ratio(&self) -> f32 { 0.5 }
    fn min_votes(&self) -> usize { 1 }
    fn check_prerequisites(&self, _cmd: &GameCommand) -> Result<(), String> {
        Ok(())
    }
    async fn on_success(&self, _target: &str) {
        let mut queue = crate::commands::NATIVE_COMMAND_QUEUE.lock().unwrap();
        if let Ok(count) = _target.to_string().parse::<i32>() {
            let command = format!("addplayerbots {count}");
            queue.push(command);
        }
    }
}

#[derive(Clone)]
pub struct NoBotsVote;

#[async_trait::async_trait]
impl VoteType for NoBotsVote {
    fn title(&self) -> String { "Remove Bots".into() }
    fn description(&self) -> String { "Vote to remove all AI bots from the server.".into() }
    fn min_ratio(&self) -> f32 { 0.5 }
    fn min_votes(&self) -> usize { 1 }
    
    fn check_prerequisites(&self, _cmd: &GameCommand) -> Result<(), String> {
        Ok(())
    }

    async fn on_success(&self, _target: &str) {
        let mut queue = crate::commands::NATIVE_COMMAND_QUEUE.lock().unwrap();
        queue.push("disablespawningbots".into());
    }
}