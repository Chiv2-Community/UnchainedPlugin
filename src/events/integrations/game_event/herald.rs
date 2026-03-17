use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;
use clap::Parser;
use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::events::broadcast::{BroadcastMessage, Notify};
use crate::events::bus::{EventPublisher, Subscriber};
use crate::events::integrations::command::Command;
use crate::events::models::{GameCommand, GameEvent};

pub struct HeraldState {
    pub mention_on_crash: bool,
    pub mention_on_admin: bool,
}

pub struct AdminHeraldSubscriber {
    state: Arc<Mutex<HeraldState>>,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl AdminHeraldSubscriber {
    pub fn new(state: Arc<Mutex<HeraldState>>, broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self { state, broadcaster }
    }
}

#[async_trait]
impl Subscriber<GameEvent> for AdminHeraldSubscriber {
    fn identifier(&self) -> &'static str {
        "AdminHeraldSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        let broadcaster = self.broadcaster;
        let state = self.state.lock().await;

        match event {
            GameEvent::CrashEvent(alert) => {
                let mut msg = BroadcastMessage::new()
                    .title("💀 SERVER CRASH".to_string())
                    .content(format!("**SERVER CRASH**: `{}` \ntrace: \n```\n{}\n```", alert.event_type, alert.event_trace.join("\n")));
                
                if state.mention_on_crash {
                    msg.notify.push(Notify::Admin);
                }
                broadcaster.publish(msg);
            }
            GameEvent::AdminAlertEvent(alert) => {
                let mut msg = BroadcastMessage::new()
                    .title("⚠️ Admin Alert".to_string())
                    .content(format!("`{}` reports: *\"{}\"*", alert.reporter, alert.reason));
                
                if state.mention_on_admin {
                    msg.notify.push(Notify::Admin);
                }
                broadcaster.publish(msg);
            }
            _ => {}
        }
    }

    async fn on_tick(&mut self) {}
}

#[derive(Parser, Clone)]
#[command(name = "cmd", about = "Execute a console command")]
pub struct CmdArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

pub struct CmdCommand;

#[async_trait]
impl Command<CmdArgs> for CmdCommand {
    fn required_permissions(&self) -> crate::events::models::PermissionFlags {
        crate::events::models::PermissionFlags::ADMIN
    }

    async fn execute(&self, args: CmdArgs, _command: &GameCommand) {
        let full_command = args.command.join(" ");
        NATIVE_COMMAND_QUEUE.lock().unwrap().push(full_command.clone());
        // We might want to acknowledge this, but where?
        // CommandSubscriber doesn't have a broadcaster for the command result itself, 
        // it expects the command to publish to a broadcaster if needed.
    }
}

#[derive(Parser, Clone)]
#[command(name = "say", about = "Send a global message to the server")]
pub struct SayArgs {
    #[arg(trailing_var_arg = true)]
    pub message: Vec<String>,
}

pub struct SayCommand {
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl SayCommand {
    pub fn new(broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self { broadcaster }
    }
}

#[async_trait]
impl Command<SayArgs> for SayCommand {
    fn required_permissions(&self) -> crate::events::models::PermissionFlags {
        crate::events::models::PermissionFlags::ADMIN
    }

    async fn execute(&self, args: SayArgs, _command: &GameCommand) {
        let message = args.message.join(" ");
        self.broadcaster.publish(BroadcastMessage::from(message));
    }
}
