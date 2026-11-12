use async_trait::async_trait;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::Subscriber;
use unicode_segmentation::UnicodeSegmentation;

pub struct InGameChatBroadcastSubscriber;

// Only localized system chat is decoded by the display hook.
const BROADCAST_PREFIX: &str = "{$";

pub(crate) fn broadcast_parts(message: &str) -> Option<(&str, &str, Option<u32>)> {
    let (title, body) = message.strip_prefix(BROADCAST_PREFIX)?.split_once("}\n")?;
    let (title, color) = match title.rsplit_once("|#") {
        Some((title, color)) => {
            if color.len() != 6 || !color.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return None;
            }
            (title, Some(u32::from_str_radix(color, 16).ok()?))
        }
        None => (title, None),
    };

    if title.is_empty() || title.trim() != title || title.chars().any(char::is_control) {
        return None;
    }

    Some((title, body, color))
}

impl InGameChatBroadcastSubscriber {
    fn encode(event: &BroadcastMessage) -> String {
        let (title, body) = event.game_chat_parts();
        let title = title.map(|title| Self::strip_emojis(&title)).unwrap_or_default();
        let body = Self::strip_emojis(&body);
        if title.is_empty() {
            body
        } else {
            let color = event.color.unwrap_or(0xFFFFFF) & 0xFFFFFF;
            format!("{BROADCAST_PREFIX}{title}|#{color:06X}}}\n{body}")
        }
    }

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
        crate::game::chivalry2::send_ingame_message(Self::encode(event), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titled_broadcast_separates_sender_from_body() {
        let message = BroadcastMessage::new()
            .title(crate::rich_text!("🗺️ **Map Change**"))
            .content("Starting match on Wardenglade.\nGood luck!")
            .field("Mode", "FFA");
        let encoded = InGameChatBroadcastSubscriber::encode(&message);
        assert_eq!(broadcast_parts(&encoded), Some(("Map Change", "Starting match on Wardenglade. Good luck! Mode: FFA", Some(0xFFFFFF))));
        assert!(encoded.starts_with("{$Map Change|#FFFFFF}\nStarting match on Wardenglade."));
        assert_eq!(message.to_plain_text(), "🗺️ Map Change\nStarting match on Wardenglade.\nGood luck!\nMode: FFA");
    }

    #[test]
    fn untitled_and_empty_title_broadcasts_remain_plain() {
        for title in [None, Some(""), Some("🗺️\n ")] {
            let mut message = BroadcastMessage::new().content("Hello\nworld");
            message.title = title.map(Into::into);
            assert_eq!(InGameChatBroadcastSubscriber::encode(&message), "Hello world");
        }
    }

    #[test]
    fn broadcast_envelope_preserves_literal_markup_and_title_delimiters() {
        let message = BroadcastMessage::new().title("Round: [End]\nNext")
            .content("<literal> & !cmd quit");
        let encoded = InGameChatBroadcastSubscriber::encode(&message);
        let (sender, body, color) = broadcast_parts(&encoded).unwrap();
        assert_eq!(sender, "Round: [End] Next");
        assert_eq!(body, message.content_ref().unwrap().to_game_chat());
        assert_eq!(color, Some(0xFFFFFF));
        assert_eq!(broadcast_parts("{$Only title}\n"), Some(("Only title", "", None)));
        assert_eq!(broadcast_parts("{$Title}\nbody\nnext line"), Some(("Title", "body\nnext line", None)));
        for malformed in [
            "Map Change\nbody", "[Unchained broadcast] title\nbody", "{title}\nbody",
            "{$title\nbody", "{$title}body", "{$title}", "{$}\nbody",
            "{$ title}\nbody", "{$title }\nbody", "{$title}\r\nbody",
            "{$title\nother}\nbody",
            "{$title|#FF}\nbody", "{$title|#GG0000}\nbody", "{$title|#1000000}\nbody",
        ] {
            assert!(broadcast_parts(malformed).is_none(), "{malformed:?}");
        }
    }

    #[test]
    fn broadcast_color_round_trips_without_becoming_sender_or_body_text() {
        let message = BroadcastMessage::new().title("Title|#123456").content("Hello").color(0x12ABEF);
        let encoded = InGameChatBroadcastSubscriber::encode(&message);
        assert_eq!(encoded, "{$Title|#123456|#12ABEF}\nHello");
        assert_eq!(broadcast_parts(&encoded), Some(("Title|#123456", "Hello", Some(0x12ABEF))));
        assert_eq!(broadcast_parts("{$Title|#12abef}\nHello"), Some(("Title", "Hello", Some(0x12ABEF))));
    }

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
