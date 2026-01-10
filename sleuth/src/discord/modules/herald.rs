use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::discord::ChatType;
use crate::discord::config::DiscordConfig;
use crate::discord::config::ModuleConfig;
use crate::discord::core::*;
use crate::discord::notifications::*;
use crate::discord::responses::*;
use crate::game::chivalry2::EChatType;
use serde::Deserialize;
use serde::Serialize;
use serenity::all::CreateAllowedMentions;
use serenity::all::CreateEmbed;
use serenity::all::{Http, ChannelId, CreateMessage, RoleId};
use std::sync::Arc;

#[derive(Deserialize, Serialize, Debug)]
#[derive(Default)]
pub struct HeraldSettings {
    mention_on_crash: bool,
    mention_on_admin: bool,
}

// impl Default for HeraldSettings {
//     fn default() -> Self {
//         Self { 
//             mention_on_crash: false
//         }
//     }
// }

impl ModuleConfig for HeraldSettings {
    fn key() -> &'static str { "Herald" }
}

pub struct AdminHerald {
    admin_role_id: RoleId,
    settings: HeraldSettings,
    ctx: crate::discord::Ctx,
}

impl AdminHerald {
    pub fn new(ctx: crate::discord::Ctx, role_id: u64) -> Self {
        Self {
            // FIXME
            admin_role_id: RoleId::new(if role_id>0 {role_id} else {1}),
            settings: ctx.config.get_module_config::<HeraldSettings>().unwrap_or_default(),
            ctx,
        }
    }

    // Helper to check if a Discord user has the required admin role
    fn is_admin(&self, roles: &[RoleId]) -> bool {
        roles.contains(&self.admin_role_id)
    }
}

#[async_trait::async_trait]
impl DiscordSubscriber for AdminHerald {    
    impl_reconfigure!(HeraldSettings);

    async fn on_event(&mut self, event: &dyn GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        let any = event.as_any();

        // 1. Listen for In-Game Admin Alerts (Game -> Discord)
        // if let Some(alert) = any.downcast_ref::<AdminAlert>() {
        //     let allowed_mentions = CreateAllowedMentions::new()
        //         .roles(vec![self.admin_role_id]);
        //     return BotResponse::from(
        //         CreateMessage::new().content(format!(
        //             "⚠️ <@&{}> **Internal Alert**: `{}` reports: *\"{}\"*",
        //             self.admin_role_id, alert.reporter, alert.reason
        //         )).allowed_mentions(allowed_mentions)
        //     ).into_responses();
        // }
        
        if let Some(alert) = any.downcast_ref::<CrashEvent>() {
            let allowed_mentions = CreateAllowedMentions::new()
                .roles(vec![self.admin_role_id]);
            // TODO: push to admin channel only
            let alert_mention = match self.settings.mention_on_crash {
                true => format!("<@&{}> ", self.admin_role_id),
                false => "".into()
            };

            return BotResponse::from(
                CreateMessage::new().content(format!(
                    "💀 {}**SERVER CRASH**: `{}` \ntrace: \n```\n{}\n```",
                    alert_mention, alert.event_type, alert.event_trace.join("\n")
                )).allowed_mentions(allowed_mentions)
            ).into_responses();
        }

        if let Some(cmd) = any.downcast_ref::<GameCommandEvent>() {
            let ensure_elevated = || {
                if !cmd.actor.is_elevated() {
                    Err(msg("🚫 You do not have permission to use this command.").into_responses())
                } else {
                    Ok(())
                }
            };
            
            match cmd.name.as_str() {
                "cmd" => {
                    if cmd.source != CommandSource::Discord { return NO_RESP; }
                    if let Err(unauthorized_resp) = ensure_elevated() {
                        return unauthorized_resp;
                    }

                    NATIVE_COMMAND_QUEUE.lock().unwrap().push(cmd.raw_args.clone());
                    return msg(format!("✅ **Executed**: {}", cmd.raw_args)).into_responses();
                },
                "say" => {                    
                    if cmd.source != CommandSource::Discord { return NO_RESP; }
                    if let Err(unauthorized_resp) = ensure_elevated() {
                        return unauthorized_resp;
                    }
                    self.ctx.chat.send(cmd.raw_args.clone(), ChatType::Admin);
                    return msg(format!("✅ **Broadcasted**: {}", cmd.raw_args)).into_responses();
                },
                "admin" => {
                    if cmd.source != CommandSource::GameChat { return NO_RESP; }
                    let allowed_mentions = CreateAllowedMentions::new()
                        .roles(vec![self.admin_role_id]);
                    let alert_mention = match self.settings.mention_on_admin {
                        true => format!("<@&{}> ", self.admin_role_id),
                        false => "".into()
                    };
                    return BotResponse::from(
                        CreateMessage::new().content(format!(
                            "⚠️ {}**Internal Alert**: `{}` reports: *\"{}\"*",
                            alert_mention, cmd.actor.display_name, cmd.raw_args
                        )).allowed_mentions(allowed_mentions)).into_responses();
                },
                _ => {}
            };
        }

        NO_RESP
    }
}