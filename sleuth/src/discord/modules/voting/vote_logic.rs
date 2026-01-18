
#[async_trait::async_trait]
pub trait VoteType: Send + Sync {
    fn title(&self) -> String;
    fn description(&self) -> String;
    fn min_ratio(&self) -> f32; // e.g., 0.5 for 50%
    fn min_votes(&self) -> usize;
    fn duration_secs(&self) -> u64;
    
    // The custom logic that runs on success
    async fn on_success(&self, target: &str);
}