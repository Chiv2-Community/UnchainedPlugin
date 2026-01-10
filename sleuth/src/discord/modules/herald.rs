use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::discord::core::*;
use crate::discord::notifications::*;
use crate::discord::responses::*;
use crate::discord::send_ingame_message;
use crate::game::chivalry2::EChatType;
use serenity::all::CreateAllowedMentions;
use serenity::all::CreateEmbed;
use serenity::all::{Http, ChannelId, CreateMessage, RoleId};
use std::sync::Arc;

pub struct AdminHerald {
    admin_role_id: RoleId,
}

impl AdminHerald {
    pub fn new(role_id: u64) -> Self {
        Self {
            admin_role_id: RoleId::new(role_id),
        }
    }

    // Helper to check if a Discord user has the required admin role
    fn is_admin(&self, roles: &[RoleId]) -> bool {
        roles.contains(&self.admin_role_id)
    }
}

#[async_trait::async_trait]
impl DiscordSubscriber for AdminHerald {
    fn name(&self) -> &'static str { "AdminHerald" }

    async fn on_event(&mut self, event: &dyn GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        let any = event.as_any();

        // 1. Listen for In-Game Admin Alerts (Game -> Discord)
        if let Some(alert) = any.downcast_ref::<AdminAlert>() {
            let allowed_mentions = CreateAllowedMentions::new()
                .roles(vec![self.admin_role_id]);
            return BotResponse::from(
                CreateMessage::new().content(format!(
                    "⚠️ <@&{}> **Internal Alert**: `{}` reports: *\"{}\"*",
                    self.admin_role_id, alert.reporter, alert.reason
                )).allowed_mentions(allowed_mentions)
            ).into_responses();
        }
        
        if let Some(alert) = any.downcast_ref::<CrashEvent>() {
            let allowed_mentions = CreateAllowedMentions::new()
                .roles(vec![self.admin_role_id]);
            // TODO: push to admin channel only
            return BotResponse::from(
                CreateMessage::new().content(format!(
                    "💀 <@&{}> **SERVER CRASH**: `{}` \ntrace: \n```\n{}\n```",
                    // "💀 {} **SERVER CRASH**: `{}` \ntrace: \n```\n{}\n```",
                    self.admin_role_id, alert.event_type, alert.event_trace.join("\n")
                )).allowed_mentions(allowed_mentions)
            ).into_responses();
        }

        // 2. Listen for Discord Commands (Discord -> Game)
        if let Some(cmd) = any.downcast_ref::<CommandRequest>() {
            if cmd.command.starts_with("!cmd ") {
                if !self.is_admin(&cmd.user_roles) {
                    return msg("🚫 You do not have permission to use herald commands.").into_responses();
                }
                let command = &cmd.command[5..]; // Strip "!cmd "
                NATIVE_COMMAND_QUEUE.lock().unwrap().push(command.to_string());
                return msg(format!("✅ **Executed**: {}", command)).into_responses();
            }
            // Only respond to "!say" if the user is an Admin
            if cmd.command.starts_with("!say ") {
                if !self.is_admin(&cmd.user_roles) {
                    return msg("🚫 You do not have permission to use herald commands.").into_responses();
                }

                let announcement = &cmd.command[5..]; // Strip "!say "

                send_ingame_message(announcement.to_string(), EChatType::Admin.into());

                return msg(format!("✅ **Broadcasted**: {}", announcement)).into_responses();
            }
        }

        NO_RESP
    }
}