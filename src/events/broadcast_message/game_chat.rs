use async_trait::async_trait;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::Subscriber;
use crate::game::chivalry2::EChatType;
use unicode_segmentation::UnicodeSegmentation;

pub struct InGameChatBroadcastSubscriber;

impl InGameChatBroadcastSubscriber {
    fn strip_emojis(s: &str) -> String {
        let mut intermediate = String::new();
        for g in s.graphemes(true) {
            if emojis::get(g).is_some() {
                intermediate.push(' ');
            } else {
                intermediate.push_str(g);
            }
        }
        
        let mut final_result = String::new();
        let mut last_was_space = false;
        for c in intermediate.chars() {
            if c.is_whitespace() {
                if !last_was_space {
                    final_result.push(' ');
                    last_was_space = true;
                }
            } else {
                final_result.push(c);
                last_was_space = false;
            }
        }
        
        final_result.trim().to_string()
    }
}

#[async_trait]
impl Subscriber<BroadcastMessage> for InGameChatBroadcastSubscriber {
    fn identifier(&self) -> &'static str {
        "InGameChatBroadcastSubscriber"
    }

    async fn on_event(&mut self, event: &BroadcastMessage) {
        let chat_message: String = event.clone().into();
        let stripped_message = Self::strip_emojis(&chat_message);
        crate::game::chivalry2::send_ingame_message(stripped_message, Some(EChatType::ServerSay));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_emojis() {
        let cases = vec![
            ("Hello 🛠️ World!", "Hello World!"),
            ("🛠️ World", "World"),
            ("Hello 🛠️", "Hello"),
            ("Hello 🛠️🎮 World", "Hello World"),
            ("Hello 🛠️  World", "Hello World"), // multiple spaces
            ("🇩🇪 Germany", "Germany"),
            ("No emoji here", "No emoji here"),
        ];

        for (input, expected) in cases {
            let result = InGameChatBroadcastSubscriber::strip_emojis(input);
            assert_eq!(result, expected, "Failed for input: {:?}", input);
        }
    }
}
