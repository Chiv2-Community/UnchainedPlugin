use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;
use clap::Parser;
use serenity::all::{ChannelId, CreateAllowedMentions, CreateMessage, Http, RoleId};
use serenity::builder::CreateEmbed;
use crate::events::broadcast::{BroadcastMessage, Notify};
use crate::events::bus::{EventPublisher, Subscriber};

use crate::commands::Command;
use crate::events::models::{AdminAlert, CommandRequest, GameEvent, PermissionFlags};
use crate::swarn;

pub struct AdminAlertModule {
    mention_on_admin: bool,
    http: Arc<Http>,
    channel_id: ChannelId,
    admin_role_id: Option<RoleId>,
    game_event_publisher: &'static EventPublisher<GameEvent>,
}

impl AdminAlertModule {
    pub fn new(
        mention_on_admin: bool,
        http: Arc<Http>,
        channel_id: ChannelId,
        admin_role_id: Option<RoleId>,
        game_event_publisher: &'static EventPublisher<GameEvent>,
    ) -> Self {
        Self {
            mention_on_admin,
            http,
            channel_id,
            admin_role_id,
            game_event_publisher,
        }
    }
}

impl Clone for AdminAlertModule {
    fn clone(&self) -> Self {
        Self {
            mention_on_admin: self.mention_on_admin,
            http: self.http.clone(),
            channel_id: self.channel_id,
            admin_role_id: self.admin_role_id,
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
            let mut msg = CreateMessage::new().embed(
                CreateEmbed::new()
                    .title("🚨 Admin Alert".to_string())
                    .description(format!("`{}` reports: *\"{}\"*", alert.reporter, alert.reason)),
            );

            if self.mention_on_admin {
                if let Some(role_id) = &self.admin_role_id {
                    msg = msg
                        .allowed_mentions(
                            CreateAllowedMentions::default().roles(vec![role_id.clone()]),
                        )
                        .content(format!("<@&{}> ", role_id));
                } else {
                    swarn!("Attempted to alert admins, but no admin role id is set.");
                }
            }

            let _ = self.channel_id.send_message(&self.http, msg).await;
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
