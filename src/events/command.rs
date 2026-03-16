use async_trait::async_trait;
use clap::{CommandFactory, Parser};
use crate::events::models::{GameCommand, GameEvent};


#[async_trait]
pub trait Command<Args: Parser + CommandFactory + Send + Sync>: Send + Sync {
    fn check_permission(&self, command: &GameCommand) -> bool { true }
    async fn execute(&self, args: Args, command: &GameCommand);
    async fn on_tick(&self) {}
    async fn on_event(&self, _event: &GameEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::sync::{Arc, Mutex};

    use crate::events::models::{ActorIdentity, ActorPermissions, CommandActor, CommandSource, PermissionFlags};

    #[derive(Parser, Clone)]
    #[command(name = "example", about = "An example command")]
    struct ExampleCommandArgs {
        message: String,
    }

    struct ExampleCommand {
        executed: Arc<Mutex<Option<String>>>,
    }

    #[async_trait]
    impl Command<ExampleCommandArgs> for ExampleCommand {
        async fn execute(&self, args: ExampleCommandArgs, _command: &GameCommand) {
            let mut executed = self.executed.lock().unwrap();
            *executed = Some(args.message);
        }
    }

    #[tokio::test]
    async fn test_example_command_execution() {
        let executed_message = Arc::new(Mutex::new(None));
        let command = ExampleCommand {
            executed: executed_message.clone(),
        };

        let args = ExampleCommandArgs {
            message: "Hello, World!".to_string(),
        };

        let game_command = GameCommand {
            name: "example".to_string(),
            args: vec!["Hello, World!".to_string()],
            raw_args: "\"Hello, World!\"".to_string(),
            actor: CommandActor {
                display_name: "Player".to_string(),
                identity: ActorIdentity::GamePlayer {
                    player_id: 1,
                    display_name: "Player".to_string(),
                },
                permissions: ActorPermissions {
                    flags: PermissionFlags::USER,
                },
            },
            source: CommandSource::GameChat,
        };

        command.execute(args, &game_command).await;

        let result = executed_message.lock().unwrap();
        assert_eq!(result.as_ref().unwrap(), "Hello, World!");
    }
}