use crate::modules::vote::{VoteType, ErasedVoteType, VoteTypeHandler};
use clap::Parser;
use tokio::spawn;
use crate::events::bus::EventPublisher;
use crate::events::models::GameEvent;

#[derive(Parser, Clone, Debug)]
#[command(name = "restart-server")]
pub struct RestartServerVoteArgs {
}

#[derive(Clone)]
pub struct RestartServerVote {
    event_publisher: &'static EventPublisher<GameEvent>,
}

impl RestartServerVote {
    pub fn new(event_publisher: &'static EventPublisher<GameEvent>) -> Self {
        Self { event_publisher }
    }
}

impl VoteType<RestartServerVoteArgs> for RestartServerVote {
    fn title(&self) -> String { "Restart Server".into() }
    fn description(&self) -> String { "Vote to restart the server.".into() }
    fn vote_description(&self, _args: RestartServerVoteArgs) -> String {
        "Vote to restart the server".into()
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.75 }
    fn min_votes_required_ratio(&self) -> f32 { 0.75 }
    fn on_success(&self, _args: RestartServerVoteArgs) {
        self.event_publisher.publish(GameEvent::ServerRestartEvent);
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            // Exit code -67 tells the launcher to restart
            std::process::exit(-67);
        });
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}
