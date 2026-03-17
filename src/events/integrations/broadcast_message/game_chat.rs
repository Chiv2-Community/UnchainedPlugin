use async_trait::async_trait;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::Subscriber;
use crate::game::chivalry2::EChatType;

pub struct InGameChatBroadcastSubscriber;

#[async_trait]
impl Subscriber<BroadcastMessage> for InGameChatBroadcastSubscriber {
    fn identifier(&self) -> &'static str {
        "InGameChatBroadcastSubscriber"
    }

    async fn on_event(&mut self, event: &BroadcastMessage) {
        let chat_message: String = event.clone().into();
        crate::game::chivalry2::send_ingame_message(chat_message, Some(EChatType::ServerSay));
    }
}
