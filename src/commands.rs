//! The command system provides a trait-based approach to defining and handling game commands.
//!
//! # Architecture
//!
//! - [`Command<Args>`]: The primary trait for defining a new command with specific arguments.
//!   It leverages [`clap`] for argument parsing and provides hooks for execution, ticks, and events.
//! - [`ErasedCommand`]: A type-erased version of the `Command` trait, allowing different commands
//!   to be stored and managed in a single collection.
//! - [`CommandHandler`]: A wrapper that bridges the gap between `Command<Args>` and `ErasedCommand`.
//!
//! # Adding New Commands
//!
//! To add a new command to the project:
//! 1. Define a struct for your command arguments and derive [`clap::Parser`] and [`clap::CommandFactory`].
//! 2. Define a struct for your command and implement `Command<MyCommandArgs>` for it.
//! 3. Register your command in the appropriate place:
//!    - **Always active**: Register in `src/features/events.rs` within `initialize_subscribers`.
//!    - **Discord only**: Register in `src/features/discord.rs` if it should only be active with Discord.
//!
//! # Registration & Execution
//!
//! Commands are registered to a [`CommandSubscriber`], which is defined in `src/modules/command.rs`.
//! The `CommandSubscriber` is initialized in `src/features/events.rs` and subscribed to the
//! central game event bus to listen for [`CommandRequest`](crate::events::models::CommandRequest) events.
use std::marker::PhantomData;
use std::sync::Mutex;
use async_trait::async_trait;
use clap::{CommandFactory, Parser};
use once_cell::sync::Lazy;
use crate::events::models::{ActorIdentity, ActorPermissions, CommandActor, CommandRequest, CommandSource, GameEvent, PermissionFlags};
use crate::features::events::EVENT_SYSTEM;

pub type CommandResult = anyhow::Result<()>;

pub static NATIVE_COMMAND_QUEUE: Lazy<Mutex<Vec<String>>> = Lazy::new(|| {
    Mutex::new(Vec::new())
});

#[cfg(feature="cli_commands")]
pub fn spawn_cli_handler() {
    std::thread::spawn(move || {
        let input_source = std::io::stdin();
        let mut buffer = String::new();
        
        while input_source.read_line(&mut buffer).is_ok() {
            let maybe_command_parts = shlex::split(buffer.trim());
            if let Some(command_parts) = maybe_command_parts {
                if !command_parts.is_empty() {
                    let command_slice = command_parts.as_slice();
                    let (command, args) = match command_slice.first().map(|s| s.as_str()) {
                        Some("help") => ("help".to_string(), command_slice[1..].to_vec()),
                        Some(name) => (name.to_string(), command_slice[1..].to_vec()),
                        None => continue,
                    };

                    EVENT_SYSTEM.game_event_publisher.publish(GameEvent::CommandRequestEvent(CommandRequest {
                        name: command,
                        args,
                        raw_args: buffer.clone(),
                        actor: CommandActor {
                            identity: ActorIdentity::ServerConsole,
                            permissions: ActorPermissions {
                                flags: PermissionFlags::all(),
                            },
                            display_name: "Server Console".to_string(),
                        },
                        source: CommandSource::ServerConsole,
                    }));
                }
            }
            buffer.clear();
        }
    });
}


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
    fn required_source(&self) -> Option<crate::events::models::CommandSource> {
        None
    }

    async fn execute(&self, args: Args, command: &CommandRequest);
    async fn on_tick(&self) {}
    async fn on_event(&self, _event: &GameEvent) {}
}

#[async_trait]
pub trait ErasedCommand: Send + Sync {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn usage(&self) -> String;
    fn required_source(&self) -> Option<crate::events::models::CommandSource>;
    fn required_permissions(&self) -> crate::events::models::PermissionFlags;
    fn help(&self) -> String;
    async fn execute(&self, command: &CommandRequest);
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

    fn required_source(&self) -> Option<crate::events::models::CommandSource> {
        self.command.required_source()
    }

    fn required_permissions(&self) -> crate::events::models::PermissionFlags {
        self.command.required_permissions()
    }

    fn help(&self) -> String {
        use crate::events::models::{CommandSource, PermissionFlags};
        let source_tag = match self.required_source() {
            Some(CommandSource::GameChat) => "🎮",
            Some(CommandSource::Discord) => "💬",
            Some(CommandSource::ServerConsole) => "🖳",
            None => "🌐",
        };
        let lock = if self.required_permissions().contains(PermissionFlags::ADMIN) { "🔒" } else { "" };

        format!("`{}` - {}{}*{}*", self.usage(), source_tag, lock, self.description())
    }

    async fn execute(&self, command: &CommandRequest) {
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

    use crate::events::models::PermissionFlags;
    use crate::test_utils::{mock_actor, TestArgs, TestCommand, mock_command_request};

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

        async fn execute(&self, args: ExampleCommandArgs, command: &CommandRequest) {
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

        let actor = mock_actor("Player");
        let req = mock_command_request(actor, "example", vec!["Hello, World!".to_string()]);

        command.execute(args, &req).await;

        let result = executed_message.lock().unwrap();
        assert_eq!(result.as_ref().unwrap(), "Hello, World!");
    }

    #[tokio::test]
    async fn test_command_handler_execution() {
        let cmd = TestCommand;
        let handler = CommandHandler::<TestCommand, TestArgs>::new(cmd);

        let actor = mock_actor("TestUser");
        let req = mock_command_request(actor, "test", vec!["hello".to_string(), "true".to_string()]);

        handler.execute(&req).await;
    }

}
