use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::events::models::ChatType;
use crate::discord::config::DiscordConfig;
use crate::discord::config::ModuleConfig;
use crate::discord::core::*;
use crate::discord::events::*;
use crate::discord::responses::*;
use serde::Deserialize;
use serde::Serialize;
use serenity::all::CreateAllowedMentions;
use serenity::all::{Http, ChannelId, CreateMessage, RoleId};
use sleuth_macros::handler_command;
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
    pub fn new(ctx: crate::discord::Ctx, role_id: Option<u64>) -> Self {
        Self {
            // FIXME
            admin_role_id: RoleId::new(role_id.unwrap_or(1)),
            settings: ctx.config.get_module_config::<HeraldSettings>().unwrap_or_default(),
            ctx,
        }
    }

    #[handler_command(name = "cmd", desc = "Execute a console command.", source = "Discord", elevated = true)]
    pub fn cmd_cmd(&mut self, _command: String, cmd: &GameCommand) -> Vec<BotResponse> {
        NATIVE_COMMAND_QUEUE.lock().unwrap().push(cmd.raw_args.clone());
        msg(format!("✅ **Executed**: {}", cmd.raw_args)).into_responses()
    }

    #[handler_command(name = "say", desc = "Send a global message to the server.", source = "Discord", elevated = true)]
    pub fn cmd_say(&mut self, _message: String, cmd: &GameCommand) -> Vec<BotResponse> {
        self.ctx.chat.send(cmd.raw_args.clone(), ChatType::Admin);
        msg(format!("✅ **Broadcasted**: {}", cmd.raw_args)).into_responses()
    }

    #[handler_command(name = "admin", desc = "Call for an admin", source = "GameChat")]
    pub fn cmd_admin(&mut self, _message: String, cmd: &GameCommand) -> Vec<BotResponse> {
        let allowed_mentions = CreateAllowedMentions::new()
            .roles(vec![self.admin_role_id]);
        let alert_mention = match self.settings.mention_on_admin {
            true => format!("<@&{}> ", self.admin_role_id),
            false => "".into()
        };
        BotResponse::from(
            CreateMessage::new().content(format!(
                "⚠️ {}**Internal Alert**: `{}` reports: *\"{}\"*",
                alert_mention, cmd.actor.display_name, cmd.raw_args
            )).allowed_mentions(allowed_mentions)).into_responses()
    }
}

#[async_trait::async_trait]
impl DiscordSubscriber for AdminHerald {  
    fn get_commands(&self) -> Vec<CommandInfo> {
        crate::auto_help!(self, [
            cmd_cmd,
            cmd_say,
            cmd_admin
        ])
    }  
    impl_reconfigure!(HeraldSettings);

    async fn on_event(&mut self, event: &GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        match event {
            GameEvent::GameCommandEvent(cmd) => {
                crate::auto_dispatch!(self, cmd, [
                    cmd_cmd,
                    cmd_say,
                    cmd_admin
                ]);
                NO_RESP
            }
            GameEvent::CrashEvent(alert) => {
                let allowed_mentions = CreateAllowedMentions::new()
                    .roles(vec![self.admin_role_id]);
                // TODO: push to admin channel only
                let alert_mention = match self.settings.mention_on_crash {
                    true => format!("<@&{}> ", self.admin_role_id),
                    false => "".into()
                };

                BotResponse::from(
                    CreateMessage::new().content(format!(
                        "💀 {}**SERVER CRASH**: `{}` \ntrace: \n```\n{}\n```",
                        alert_mention, alert.event_type, alert.event_trace.join("\n")
                    )).allowed_mentions(allowed_mentions)
                ).into_responses()
            }
            _ => NO_RESP,
        }
    }
}