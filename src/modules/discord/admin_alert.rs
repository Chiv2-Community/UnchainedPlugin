use std::sync::Arc;
use async_trait::async_trait;
use clap::Parser;
use serenity::all::{CreateAllowedMentions, CreateMessage, Http, RoleId};
use serenity::builder::CreateEmbed;
use crate::events::bus::{EventPublisher, Subscriber};
use crate::features::discord::{send_to_channel, SharedDiscordConfig};

use crate::commands::Command;
use crate::events::models::{AdminAlert, CommandRequest, GameEvent, PermissionFlags};
use crate::swarn;

pub struct AdminAlertModule {
    http: Arc<Http>,
    discord_config: SharedDiscordConfig,
    game_event_publisher: &'static EventPublisher<GameEvent>,
}

impl AdminAlertModule {
    pub fn new(
        http: Arc<Http>,
        discord_config: SharedDiscordConfig,
        game_event_publisher: &'static EventPublisher<GameEvent>,
    ) -> Self {
        Self {
            http,
            discord_config,
            game_event_publisher,
        }
    }
}

impl Clone for AdminAlertModule {
    fn clone(&self) -> Self {
        Self {
            http: self.http.clone(),
            discord_config: self.discord_config.clone(),
            game_event_publisher: self.game_event_publisher,
        }
    }
}

#[async_trait]
impl Subscriber<GameEvent> for AdminAlertModule {
    fn identifier(&self) -> &'static str {
        "AdminAlertSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        if let GameEvent::AdminAlertEvent(alert) = event {
            let config = self.discord_config.read().await.clone();
            let mut msg = CreateMessage::new().embed(
                CreateEmbed::new()
                    .title("🚨 Admin Alert".to_string())
                    .description(format!("`{}` reports: *\"{}\"*", alert.reporter, alert.reason)),
            );

            if config.mention_on_admin {
                if let Some(role_id) = config.admin_role_id.map(RoleId::new) {
                    msg = msg
                        .allowed_mentions(
                            CreateAllowedMentions::default().roles(vec![role_id]),
                        )
                        .content(format!("<@&{}> ", role_id));
                } else {
                    swarn!("Attempted to alert admins, but no admin role id is set.");
                }
            }

            send_to_channel(
                &self.http,
                config.admin_notification_channel_id,
                msg,
            )
            .await;
        }
    }
}

#[derive(Parser, Clone)]
#[command(name = "admin-alert", about = "Notify the admins in discord")]
pub struct AdminAlertArgs {
    #[arg(trailing_var_arg = true)]
    pub message: Vec<String>,
}

#[async_trait]
impl Command<AdminAlertArgs> for AdminAlertModule {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::empty()
    }

    async fn execute(&self, args: AdminAlertArgs, command: &CommandRequest) {
        self.game_event_publisher
            .publish(GameEvent::AdminAlertEvent(AdminAlert {
                reporter: command.actor.display_name.clone(),
                reason: args.message.join(" "),
            }));
    }
}
