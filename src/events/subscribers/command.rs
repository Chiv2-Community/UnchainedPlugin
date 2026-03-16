use async_trait::async_trait;
use clap::{Parser, CommandFactory};
use crate::events::bus::Subscriber;
use crate::events::models::{GameEvent, GameCommand};

use std::marker::PhantomData;
use crate::events::broadcast::ServerBroadcast;
use crate::events::command::Command;

#[async_trait]
pub trait ErasedCommand: Send + Sync {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn check_permission(&self, command: &GameCommand) -> bool;
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
        Args::command().get_name().to_string()
    }

    fn description(&self) -> String {
        Args::command().get_about().map(|a| a.to_string()).unwrap_or_default()
    }

    fn check_permission(&self, command: &GameCommand) -> bool {
        self.command.check_permission(command)
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

pub struct CommandSubscriber {
    commands: Vec<Box<dyn ErasedCommand>>,
    broadcaster: Box<dyn ServerBroadcast>,
}

impl CommandSubscriber {
    pub fn new(broadcaster: Box<dyn ServerBroadcast>) -> Self {
        Self {
            commands: Vec::new(),
            broadcaster,
        }
    }

    pub fn register_command<T, Args>(&mut self, command: T)
    where
        T: Command<Args> + 'static,
        Args: Parser + CommandFactory + Send + Sync + 'static,
    {
        self.commands.push(Box::new(CommandHandler::new(command)));
    }
}

#[async_trait]
impl Subscriber<GameEvent> for CommandSubscriber {
    fn identifier(&self) -> &'static str {
        "CommandSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        for command in &self.commands {
            command.on_event(event).await;
            if let GameEvent::GameCommandEvent(cmd) = event {
                if command.name() == cmd.name {
                    if command.check_permission(cmd) {
                        command.execute(cmd).await;
                    } else {
                        self
                            .broadcaster
                            .broadcast(format!("Error: User '{}' is not allowed to execute command '{}'.", cmd.actor.display_name, cmd.name).into()).await;
                    }
                }
            }
        }
    }

    fn tick_delay(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(1))
    }

    async fn on_tick(&mut self) {
        for command in &self.commands {
            command.on_tick().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use crate::events::models::{CommandActor, ActorIdentity, ActorPermissions, PermissionFlags, CommandSource};
    use crate::events::broadcast::{BroadcastMessage, ServerBroadcast};

    struct MockBroadcaster {
        messages: Arc<Mutex<Vec<BroadcastMessage>>>,
    }

    #[async_trait]
    impl ServerBroadcast for MockBroadcaster {
        async fn broadcast(&self, message: BroadcastMessage) {
            self.messages.lock().await.push(message);
        }
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
                flags: PermissionFlags::USER,
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

    #[tokio::test]
    async fn test_command_subscriber_unknown_command() {
        let broadcaster = Box::new(MockBroadcaster { messages: Arc::new(Mutex::new(Vec::new())) });
        let mut subscriber = CommandSubscriber::new(broadcaster);
        subscriber.register_command::<TestCommand, TestArgs>(TestCommand);
        
        let actor = mock_actor("TestUser");
        let game_cmd = GameCommand {
            name: "unknown".to_string(),
            args: vec!["what".to_string()],
            raw_args: "what".to_string(),
            actor,
            source: CommandSource::GameChat,
        };
        
        let event = GameEvent::GameCommandEvent(game_cmd);
        
        // This should not panic and should not execute TestCommand
        subscriber.on_event(&event).await;
    }

    #[tokio::test]
    async fn test_command_handler_parsing_failure() {
        let cmd = TestCommand;
        let handler = CommandHandler::<TestCommand, TestArgs>::new(cmd);
        
        let actor = mock_actor("TestUser");
        let game_cmd = GameCommand {
            name: "test".to_string(),
            // Only one argument, but TestArgs expects two (message and flag)
            args: vec!["hello".to_string()],
            raw_args: "hello".to_string(),
            actor,
            source: CommandSource::GameChat,
        };

        // This should print "Failed to parse command arguments" but not panic
        handler.execute(&game_cmd).await;
    }

    #[derive(Parser, Debug)]
    #[command(name = "setbool", about = "sets a bool when executed")]
    struct SetBoolArgs {}

    struct SetBoolCommand {
        pub executed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    #[async_trait]
    impl Command<SetBoolArgs> for SetBoolCommand {
        async fn execute(&self, _args: SetBoolArgs, _command: &GameCommand) {
            self.executed.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn test_command_subscriber_multiple_commands() {
        let broadcaster = Box::new(MockBroadcaster { messages: Arc::new(Mutex::new(Vec::new())) });
        let mut subscriber = CommandSubscriber::new(broadcaster);
        let setbool_executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        
        subscriber.register_command::<TestCommand, TestArgs>(TestCommand);
        subscriber.register_command::<SetBoolCommand, SetBoolArgs>(SetBoolCommand {
            executed: setbool_executed.clone()
        });
        
        let actor = mock_actor("TestUser");
        let game_cmd = GameCommand {
            name: "setbool".to_string(),
            args: vec![],
            raw_args: "".to_string(),
            actor,
            source: CommandSource::GameChat,
        };
        
        let event = GameEvent::GameCommandEvent(game_cmd);
        subscriber.on_event(&event).await;
        
        assert!(setbool_executed.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_command_tick() {
        #[derive(Parser, Debug)]
        #[command(name = "tick")]
        struct TickArgs {}

        struct TickCommand {
            ticked: std::sync::Arc<std::sync::atomic::AtomicBool>,
        }
        #[async_trait]
        impl Command<TickArgs> for TickCommand {
            async fn execute(&self, _args: TickArgs, _command: &GameCommand) {}
            async fn on_tick(&self) {
                self.ticked.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let ticked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let broadcaster = Box::new(MockBroadcaster { messages: Arc::new(Mutex::new(Vec::new())) });
        let mut subscriber = CommandSubscriber::new(broadcaster);
        subscriber.register_command::<TickCommand, TickArgs>(TickCommand { ticked: ticked.clone() });

        subscriber.on_tick().await;
        assert!(ticked.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_command_permission_denied() {
        #[derive(Parser, Debug)]
        #[command(name = "restricted")]
        struct RestrictedArgs {}

        struct RestrictedCommand {
            executed: std::sync::Arc<std::sync::atomic::AtomicBool>,
        }

        #[async_trait]
        impl Command<RestrictedArgs> for RestrictedCommand {
            fn check_permission(&self, _command: &GameCommand) -> bool { false }
            async fn execute(&self, _args: RestrictedArgs, _command: &GameCommand) {
                self.executed.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let executed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let broadcaster = Box::new(MockBroadcaster { messages: Arc::new(Mutex::new(Vec::new())) });
        let mut subscriber = CommandSubscriber::new(broadcaster);
        subscriber.register_command::<RestrictedCommand, RestrictedArgs>(RestrictedCommand {
            executed: executed.clone(),
        });

        let actor = mock_actor("TestUser");
        let game_cmd = GameCommand {
            name: "restricted".to_string(),
            args: vec![],
            raw_args: "".to_string(),
            actor,
            source: CommandSource::GameChat,
        };

        let event = GameEvent::GameCommandEvent(game_cmd);
        subscriber.on_event(&event).await;

        assert!(!executed.load(std::sync::atomic::Ordering::SeqCst));
    }
}
