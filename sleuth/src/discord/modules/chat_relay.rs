use crate::discord::responses::*;
use crate::discord::{core::*, send_ingame_message};
use crate::discord::notifications::{CommandRequest, GameChatMessage};
use crate::game::chivalry2::EChatType;
use crate::game::engine::FText;
use crate::sinfo;
use crate::ue::{FString}; // Assuming these are your internal types
use serenity::all::{Http, ChannelId, CreateMessage};
use std::sync::Arc;
use crate::resolvers::admin_control::o_FText_AsCultureInvariant;
use crate::resolvers::messages::o_BroadcastLocalizedChat;
use crate::resolvers::etc_hooks::o_GetTBLGameMode;

pub struct ChatRelayModule;

impl ChatRelayModule {
    pub fn new() -> Self {
        Self
    }

    /// Safely invokes Unreal Engine functions using the TRY_CALL_ORIGINAL macro
    fn relay_to_unreal(&self, message: String) {
        send_ingame_message(message, None);
        // if let Some(world) = crate::globals().world() {
        //     let mut settings_fstring = FString::from(message.as_str());
        //     let mut txt = FText::default();

        //     unsafe {
        //         // 1. Convert String to Culture Invariant FText
        //         // This uses your internal macro to call the hooked/original Unreal function
        //         let res = TRY_CALL_ORIGINAL!(FText_AsCultureInvariant(&mut txt, &mut settings_fstring));

        //         // 2. Get the TBLGameMode pointer
        //         let game_mode = TRY_CALL_ORIGINAL!(GetTBLGameMode(world));

        //         if !game_mode.is_null() {
        //             // 3. Broadcast to the game instance
        //             // We pass the result pointer if required by your specific macro signature
        //             TRY_CALL_ORIGINAL!(BroadcastLocalizedChat(game_mode, res, EChatType::AllSay));
        //         }
        //     }
        // }
    }
}

#[async_trait::async_trait]
impl DiscordSubscriber for ChatRelayModule {
    fn name(&self) -> &'static str { "ChatRelayModule" }

    async fn on_event(&mut self, event: &dyn GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        let any = event.as_any();
        sinfo!(f; "ChatRelayModule::on_event {:#?}", event.event_type());

        // --- DISCORD -> GAME ---
        if let Some(msg) = any.downcast_ref::<CommandRequest>() {
            // Filter out bot commands so they don't clutter in-game chat
            if !msg.command.starts_with('!') {
                let formatted_text = format!("<D>{}: {}", msg.user, msg.command);
                self.relay_to_unreal(formatted_text);
            }
            return NO_RESP;
        }

        // --- GAME -> DISCORD ---
        if let Some(game_msg) = any.downcast_ref::<GameChatMessage>() {
            if game_msg.message.starts_with("!") { return NO_RESP; }
            return msg(format!("💬 **{}**: {}", game_msg.sender, game_msg.message))
            .into_responses();
        }

        vec![]
    }
}