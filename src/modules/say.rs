use async_trait::async_trait;
use clap::Parser;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::EventPublisher;

use crate::commands::Command;
use crate::events::models::{CommandRequest, PermissionFlags};

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
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::ADMIN
    }

    fn group(&self) -> String {
        "Administration".to_string()
    }

    async fn execute(&self, args: SayArgs, _command: &CommandRequest) {
        let message = args.message.join(" ");
        self.broadcaster.publish(BroadcastMessage::from(message));
    }
}
