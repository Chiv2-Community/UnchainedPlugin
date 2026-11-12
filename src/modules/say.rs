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
        let mut broadcast = BroadcastMessage::from(message);
        if command.source == CommandSource::ServerConsole {
            broadcast = broadcast.title("Server Console").color(0x00FFFF);
        }
        self.broadcaster.publish(broadcast);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{mock_actor, mock_command_request};

    #[test_case::test_case(CommandSource::ServerConsole, Some("Server Console"), Some(0x00FFFF); "server_console")]
    #[test_case::test_case(CommandSource::Discord, None, None; "discord")]
    #[test_case::test_case(CommandSource::GameChat, None, None; "game_chat")]
    #[tokio::test]
    async fn say_presentation_follows_source(source: CommandSource, title: Option<&str>, color: Option<u32>) {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let command = SayCommand::new(Box::leak(Box::new(EventPublisher::new(sender))));
        let mut request = mock_command_request(mock_actor("Admin"), "say", vec![]);
        request.source = source;
        command.execute(SayArgs { message: vec!["<hello>".into(), "**world**".into()] }, &request).await;
        let broadcast = receiver.try_recv().unwrap();
        assert_eq!(broadcast.title.as_ref().map(|title| title.to_plain_text()), title.map(str::to_owned));
        assert_eq!(broadcast.color, color);
        assert_eq!(broadcast.content_ref().unwrap().to_plain_text(), "<hello> **world**");
    }
}
