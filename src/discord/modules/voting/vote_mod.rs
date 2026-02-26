use crate::discord::modules::voting::vote_module::VoteType;

pub struct ModVote;

#[async_trait::async_trait]
impl VoteType for ModVote {
    fn title(&self) -> String { "Toggle Mod".into() }
    fn description(&self) -> String { "Vote to enable or disable a game modifier.".into() }
    fn min_ratio(&self) -> f32 { 0.66 }
    fn min_votes(&self) -> usize { 3 }

    fn clone_box(&self) -> Box<dyn VoteType> {
        Box::new(ModVote) 
    }

    async fn on_success(&self, _target: &str) {
        // let _queue = NATIVE_COMMAND_QUEUE.lock().unwrap();
        // TODO: implement
        // find mod by name
        // spawn actor
    }
}