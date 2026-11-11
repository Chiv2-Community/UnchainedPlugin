use async_trait::async_trait;
use clap::Parser;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::EventPublisher;

use crate::commands::Command;
use crate::events::models::{CommandRequest, CommandSource, PermissionFlags};

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

    async fn execute(&self, args: SayArgs, command: &CommandRequest) {
        let message = args.message.join(" ");
        let mut broadcast = BroadcastMessage::from(crate::rich_text!("{}", message));
        if command.source == CommandSource::ServerConsole {
            broadcast = broadcast.title("Server Console");
        }
        self.broadcaster.publish(broadcast);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{mock_actor, mock_command_request};

    #[tokio::test]
    async fn console_say_uses_title_without_changing_other_sources_or_literal_text() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let command = SayCommand::new(Box::leak(Box::new(EventPublisher::new(sender))));
        for source in [CommandSource::ServerConsole, CommandSource::Discord, CommandSource::GameChat] {
            let mut request = mock_command_request(mock_actor("Admin"), "say", vec![]);
            request.source = source.clone();
            command.execute(SayArgs { message: vec!["<hello>".into(), "**world**".into()] }, &request).await;
            let broadcast = receiver.recv().await.unwrap();
            assert_eq!(broadcast.title.as_ref().map(|title| title.to_plain_text()),
                (source == CommandSource::ServerConsole).then(|| "Server Console".to_string()));
            assert_eq!(broadcast.content_ref().unwrap().to_plain_text(), "<hello> **world**");
        }
    }
}
