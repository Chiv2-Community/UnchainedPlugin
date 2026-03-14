use crate::discord::{modules::voting::vote_module::VoteType, events::GameCommand};

#[derive(Clone)]
pub struct SpeedVote;

#[async_trait::async_trait]
impl VoteType for SpeedVote {
    fn title(&self) -> String { "Set game speed".into() }
    fn description(&self) -> String { "Vote to set the game speed.".into() }
    fn min_ratio(&self) -> f32 { 0.6 }
    fn min_votes(&self) -> usize { 3 }
    fn check_prerequisites(&self, _cmd: &GameCommand) -> Result<(), String> {
        Ok(())
    }
    async fn on_success(&self, _target: &str) {
        let mut queue = crate::commands::NATIVE_COMMAND_QUEUE.lock().unwrap();

        if let Ok(speed) = _target.to_string().parse::<f32>() {
            let command = format!("slomo {speed}");
            queue.push(command);
        }
    }
}