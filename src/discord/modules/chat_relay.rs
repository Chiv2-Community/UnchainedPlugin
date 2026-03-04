use crate::discord::{ChatType, responses::*};
use crate::discord::core::DiscordSubscriber;
use crate::discord::events::GameEvent;
use crate::sinfo;
use serenity::all::{Http, ChannelId};
use std::sync::Arc;

pub struct ChatRelayModule {
    ctx: crate::discord::Ctx
}

impl ChatRelayModule {
    pub fn new(ctx: crate::discord::Ctx) -> Self {
        Self {
            ctx
        }
    }

    /// Safely invokes Unreal Engine functions using the TRY_CALL_ORIGINAL macro
    fn relay_to_unreal(&self, message: String) {
        self.ctx.chat.send(message, ChatType::Global);
        // send_ingame_message(message, None);
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

    async fn on_event(&mut self, event: &GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        sinfo!(f; "ChatRelayModule::on_event {:#?}", event.event_type());

        match event {
            // --- DISCORD -> GAME ---
            GameEvent::CommandRequestEvent(msg) => {
                // Filter out bot commands so they don't clutter in-game chat
                if !msg.command.starts_with('!') {
                    let formatted_text = format!("<D>{}: {}", msg.user, msg.command);
                    self.relay_to_unreal(formatted_text);
                }
                NO_RESP
            }
            // --- GAME -> DISCORD ---
            GameEvent::GameChatMessageEvent(game_msg) => {
                if game_msg.message.starts_with('!') {
                    NO_RESP
                } else {
                    msg(format!("💬 **{}**: {}", game_msg.sender, game_msg.message))
                        .into_responses()
                }
            }
            _ => NO_RESP,
        }
    }
}