use async_trait::async_trait;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::Subscriber;

pub struct ConsoleChatBroadcastSubscriber;

#[async_trait]
impl Subscriber<BroadcastMessage> for ConsoleChatBroadcastSubscriber {
    fn identifier(&self) -> &'static str {
        "ConsoleChatBroadcastSubscriber"
    }

    async fn on_event(&mut self, event: &BroadcastMessage) {
        let message: String = event.clone().into();
        println!("[BROADCAST] {}", message);
    }
}
