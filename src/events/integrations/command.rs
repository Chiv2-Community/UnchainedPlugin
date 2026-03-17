use std::marker::PhantomData;
use async_trait::async_trait;
use clap::{CommandFactory, Parser};
use crate::events::models::{GameCommand, GameEvent};


#[async_trait]
pub trait Command<Args: Parser + CommandFactory + Send + Sync>: Send + Sync {
    fn name(&self) -> String {
        Args::command().get_name().to_string()
    }
    fn description(&self) -> String {
        Args::command().get_about().map(|a| a.to_string()).unwrap_or_default()
    }
    fn usage(&self) -> String {
        let cmd = Args::command();
        let name = cmd.get_name();
        let mut usage = format!("!{}", name);
        for arg in cmd.get_positionals() {
            usage.push_str(&format!(" <{}>", arg.get_id()));
        }
        usage
    }
    fn required_permissions(&self) -> crate::events::models::PermissionFlags;
    fn source(&self) -> Option<crate::events::models::CommandSource> {
        None
    }

    async fn execute(&self, args: Args, command: &GameCommand);
    async fn on_tick(&self) {}
    async fn on_event(&self, _event: &GameEvent) {}
}

#[async_trait]
pub trait ErasedCommand: Send + Sync {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn usage(&self) -> String;
    fn source(&self) -> Option<crate::events::models::CommandSource>;
    fn required_permissions(&self) -> crate::events::models::PermissionFlags;
    fn help(&self) -> String;
    async fn execute(&self, command: &GameCommand);
    async fn on_tick(&self);
    async fn on_event(&self, _event: &GameEvent) {}
}

pub struct CommandHandler<T, Args> {
    command: T,
    _phantom: PhantomData<Args>,
}

impl<T, Args> CommandHandler<T, Args>
where
    T: Command<Args>,
    Args: Parser + Send + Sync,
{
    pub fn new(command: T) -> Self {
        Self {
            command,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<T, Args> ErasedCommand for CommandHandler<T, Args>
where
    T: Command<Args> + Send + Sync,
    Args: Parser + CommandFactory + Send + Sync,
{
    fn name(&self) -> String {
        self.command.name()
    }

    fn description(&self) -> String {
        self.command.description()
    }

    fn usage(&self) -> String {
        self.command.usage()
    }

    fn source(&self) -> Option<crate::events::models::CommandSource> {
        self.command.source()
    }

    fn required_permissions(&self) -> crate::events::models::PermissionFlags {
        self.command.required_permissions()
    }

    fn help(&self) -> String {
        use crate::events::models::{CommandSource, PermissionFlags};
        let source_tag = match self.source() {
            Some(CommandSource::GameChat) => "🎮",
            Some(CommandSource::Discord) => "💬",
            _ => "",
        };
        let lock = if self.required_permissions().contains(PermissionFlags::ADMIN) { "🔒" } else { "" };

        format!("`{}` - {}{}*{}*", self.usage(), source_tag, lock, self.description())
    }

    async fn execute(&self, command: &GameCommand) {
        let mut args = vec![command.name.clone()];
        args.extend(command.args.clone());

        match Args::try_parse_from(args) {
            Ok(parsed) => {
                self.command.execute(parsed, command).await;
            }
            Err(e) => {
                println!("Failed to parse command arguments for {}: {}", command.name, e);
            }
        }
    }

    async fn on_tick(&self) {
        self.command.on_tick().await;
    }

    async fn on_event(&self, event: &GameEvent) {
        self.command.on_event(event).await;
    }
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
        fn required_permissions(&self) -> PermissionFlags {
            PermissionFlags::USER
        }

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

    /// Sample command using clap for parsing
    #[derive(Parser, Debug)]
    #[command(name = "test", about = "A test command using clap")]
    struct TestArgs {
        /// A test argument (positional)
        message: String,

        /// A flag (positional)
        flag: String,
    }

    pub struct TestCommand;

    #[async_trait]
    impl Command<TestArgs> for TestCommand {
        fn required_permissions(&self) -> PermissionFlags {
            PermissionFlags::USER | PermissionFlags::START_VOTE
        }

        async fn execute(&self, args: TestArgs, _command: &GameCommand) {
            println!("Test command executed with message: {} and flag: {}", args.message, args.flag);
        }
    }

    fn mock_actor(name: &str) -> CommandActor {
        CommandActor {
            display_name: name.to_string(),
            identity: ActorIdentity::GamePlayer {
                player_id: 123,
                display_name: name.to_string(),
            },
            permissions: ActorPermissions {
                flags: PermissionFlags::USER | PermissionFlags::START_VOTE,
            },
        }
    }

    #[tokio::test]
    async fn test_command_handler_execution() {
        let cmd = TestCommand;
        let handler = CommandHandler::<TestCommand, TestArgs>::new(cmd);

        let actor = mock_actor("TestUser");
        let game_cmd = GameCommand {
            name: "test".to_string(),
            args: vec!["hello".to_string(), "true".to_string()],
            raw_args: "hello true".to_string(),
            actor,
            source: CommandSource::GameChat,
        };

        // This will print to stdout, but we are testing that it doesn't panic and handles parsing
        handler.execute(&game_cmd).await;
    }

}