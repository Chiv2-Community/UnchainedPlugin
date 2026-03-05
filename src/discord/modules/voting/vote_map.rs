use crate::{commands::NATIVE_COMMAND_QUEUE, discord::modules::voting::vote_module::VoteType};

#[derive(Clone)]
pub struct MapVote;

#[async_trait::async_trait]
impl VoteType for MapVote {
    fn title(&self) -> String { "Map Change".into() }
    fn description(&self) -> String { "Vote to change the server to a new map.".into() }
    fn min_ratio(&self) -> f32 { 0.5 }
    fn min_votes(&self) -> usize { 1 }

    async fn on_success(&self, target: &str) {
        let mut queue = NATIVE_COMMAND_QUEUE.lock().unwrap();
        queue.push(format!("servertravel {}", target));
    }
}