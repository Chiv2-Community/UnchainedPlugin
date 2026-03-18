//! The command subscriber is the central hub for handling game commands.
//!
//! It implements the [`Subscriber`] trait for [`GameEvent`] and listens specifically
//! for [`CommandRequest`](crate::events::models::CommandRequest) events.
//!
//! # Architecture
//!
//! - [`CommandSubscriber`]: Manages a collection of registered commands and dispatches requests to them.
//! - [`ErasedCommand`]: The type-erased trait used to store different commands in a single collection.
//!
//! # Integration
//!
//! The `CommandSubscriber` is a key component of the event system, typically initialized
//! in `src/features/events.rs`. It bridges the gap between the low-level event bus and
//! the high-level [`Command`](crate::commands::Command) trait definitions.
//!
//! For more details on how to define commands, see the documentation in [`src/commands.rs`](crate::commands).
//!
//! # Execution Flow
//!
//! 1. A `CommandRequest` is published to the `game_event_bus`.
//! 2. The `CommandSubscriber` receives the event via `on_event`.
//! 3. It identifies the target command by name.
//! 4. It verifies permissions and execution source.
//! 5. It executes the command and publishes results to the [`BroadcastMessage`] bus.
use async_trait::async_trait;
use clap::{Parser, CommandFactory};
use crate::events::bus::{Subscriber, EventPublisher};
use crate::events::models::{GameEvent, CommandExecuted, CommandRequest};
use crate::events::broadcast::BroadcastMessage;
use crate::features::events::EVENT_SYSTEM;

use std::marker::PhantomData;
use crate::commands::{Command, CommandHandler, ErasedCommand};
use crate::sinfo;

pub struct CommandSubscriber {
    commands: Vec<Box<dyn ErasedCommand>>,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
    game_event_publisher: &'static EventPublisher<GameEvent>,
}

impl CommandSubscriber {
    pub fn new(
        broadcaster: &'static EventPublisher<BroadcastMessage>,
        game_event_publisher: &'static EventPublisher<GameEvent>,
    ) -> Self {
        Self {
            commands: Vec::new(),
            broadcaster,
            game_event_publisher,
        }
    }

    pub fn get_registered_commands(&self) -> Vec<&dyn ErasedCommand> {
        self.commands.iter().map(|c| c.as_ref()).collect()
    }

    pub fn register<T, Args>(&mut self, command: T)
    where
        T: Command<Args> + 'static,
        Args: Parser + CommandFactory + Send + Sync + 'static,
    {
        self.commands.push(Box::new(CommandHandler::new(command)));
    }

    fn broadcast_help(&self) {
        let mut help_msg = BroadcastMessage::new()
            .title("🛠️ Sleuth System Help".to_string())
            .content("All available commands:".to_string())
            .color(0x3498db)
            .footer("💬-Discord Only | 🎮-Game Only | 🖳-Server Console Only | 🌐-Executable from anywhere | 🔒-Admin only".to_string());

        let mut field_text = String::new();
        for command in &self.commands {
            if !field_text.is_empty() {
                field_text.push('\n');
            }
            field_text.push_str(&command.help());
        }

        if !field_text.is_empty() {
            help_msg = help_msg.field("Commands".to_string(), field_text);
            self.broadcaster.publish(help_msg);
        }
    }

    async fn handle_command(&self, command: &Box<dyn ErasedCommand>, req: &CommandRequest) {
        if let Some(required_source) = command.required_source() {
            if required_source != req.source {
                self
                    .broadcaster
                    .publish(format!("Error: User '{}' attempted to execute command '{}' from {}, but it must be executed from {}", req.actor.display_name, req.name, req.source, required_source).into());
                return;
            }
        }

        if !req.actor.permissions.flags.contains(command.required_permissions()) {
            self
                .broadcaster
                .publish(format!("Error: User '{}' is not allowed to execute command '{}'.", req.actor.display_name, req.name).into());
            return;
        }

        sinfo!("User '{}' executed command '{} {}'", req.actor.display_name, req.name, req.raw_args);
        command.execute(req).await;
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
        }

        if let GameEvent::CommandRequestEvent(req) = event {
            if req.name == "help" {
                self.broadcast_help();
                return;
            }

            let maybe_command =
                self.commands.iter().find(|c| c.name() == req.name);

            if maybe_command.is_none() {
                self.broadcaster.publish(format!("User '{}' attempted to execute non-existent command '{}'.", req.actor.display_name, req.name).into());
                return;
            }

            self.handle_command(maybe_command.unwrap(), req).await;

            let cmd = CommandExecuted {
                name: req.name.clone(),
                args: req.args.clone(),
                raw_args: req.raw_args.clone(),
                actor: req.actor.clone(),
                source: req.source.clone(),
            };
            self.game_event_publisher.publish(GameEvent::CommandExecutedEvent(cmd));
        }
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
    use crate::events::broadcast::BroadcastMessage;
    use crate::events::bus::EventPublisher;
    use crate::test_utils::{mock_actor, mock_actor_elevated, mock_broadcaster, mock_game_event_publisher, TestArgs, TestCommand, mock_command_request};

    #[tokio::test]
    async fn test_command_subscriber_unknown_command() {
        let (broadcaster, _messages) = mock_broadcaster();
        let (game_event_pub, _events) = mock_game_event_publisher();
        let mut subscriber = CommandSubscriber::new(broadcaster, game_event_pub);
        subscriber.register::<TestCommand, TestArgs>(TestCommand);
        
        let actor = mock_actor("TestUser");
        let req = mock_command_request(actor, "unknown", vec!["what".to_string()]);
        let event = GameEvent::CommandRequestEvent(req);
        
        // This should not panic and should not execute TestCommand
        subscriber.on_event(&event).await;
    }

    #[tokio::test]
    async fn test_command_handler_parsing_failure() {
        let cmd = TestCommand;
        let handler = CommandHandler::<TestCommand, TestArgs>::new(cmd);
        
        let actor = mock_actor("TestUser");
        let req = mock_command_request(actor, "test", vec!["hello".to_string()]);

        // This should print "Failed to parse command arguments" but not panic
        handler.execute(&req).await;
    }

    #[derive(Parser, Debug)]
    #[command(name = "setbool", about = "sets a bool when executed")]
    struct SetBoolArgs {}

    struct SetBoolCommand {
        pub executed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    #[async_trait]
    impl Command<SetBoolArgs> for SetBoolCommand {
        fn required_permissions(&self) -> PermissionFlags {
            PermissionFlags::START_VOTE
        }

        async fn execute(&self, _args: SetBoolArgs, command: &CommandRequest) {
            self.executed.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn test_command_subscriber_multiple_commands() {
        let (broadcaster, _messages) = mock_broadcaster();
        let (game_event_pub, _events) = mock_game_event_publisher();
        let mut subscriber = CommandSubscriber::new(broadcaster, game_event_pub);
        let setbool_executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        
        subscriber.register::<TestCommand, TestArgs>(TestCommand);
        subscriber.register::<SetBoolCommand, SetBoolArgs>(SetBoolCommand {
            executed: setbool_executed.clone()
        });
        
        let actor = mock_actor("TestUser");
        let req = mock_command_request(actor, "setbool", vec![]);
        let event = GameEvent::CommandRequestEvent(req);
        subscriber.on_event(&event).await;
        
        assert!(setbool_executed.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_command_tick() {
        #[derive(Parser, Debug)]
        #[command(name = "tick")]
        struct TickArgs {}

        struct TickCommand {
            ticked: Arc<std::sync::atomic::AtomicBool>,
        }
        #[async_trait]
        impl Command<TickArgs> for TickCommand {
            fn required_permissions(&self) -> PermissionFlags {
                PermissionFlags::USER
            }

            async fn execute(&self, _args: TickArgs, command: &CommandRequest) {}
            async fn on_tick(&self) {
                self.ticked.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let ticked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (broadcaster, _messages) = mock_broadcaster();
        let (game_event_pub, _events) = mock_game_event_publisher();
        let mut subscriber = CommandSubscriber::new(broadcaster, game_event_pub);
        subscriber.register::<TickCommand, TickArgs>(TickCommand { ticked: ticked.clone() });

        subscriber.on_tick().await;
        assert!(ticked.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_command_permission_denied() {
        #[derive(Parser, Debug)]
        #[command(name = "admin_only")]
        struct AdminOnlyArgs {}

        struct AdminOnlyCommand {
            executed: std::sync::Arc<std::sync::atomic::AtomicBool>,
        }

        #[async_trait]
        impl Command<AdminOnlyArgs> for AdminOnlyCommand {
            fn required_permissions(&self) -> PermissionFlags {
                PermissionFlags::ADMIN
            }
            async fn execute(&self, _args: AdminOnlyArgs, _command: &CommandRequest) {
                self.executed.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let executed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (broadcaster, _messages) = mock_broadcaster();
        let (game_event_pub, _events) = mock_game_event_publisher();
        let mut subscriber = CommandSubscriber::new(broadcaster, game_event_pub);
        subscriber.register::<AdminOnlyCommand, AdminOnlyArgs>(AdminOnlyCommand {
            executed: executed.clone(),
        });

        let actor = mock_actor_elevated("NormalUser", false); 
        let req = mock_command_request(actor, "admin_only", vec![]);
        let event = GameEvent::CommandRequestEvent(req);
        subscriber.on_event(&event).await;

        assert!(!executed.load(std::sync::atomic::Ordering::SeqCst), "Command should not execute for user without ADMIN permission");
    }

    #[tokio::test]
    async fn test_command_permission_allowed() {
        #[derive(Parser, Debug)]
        #[command(name = "user_command")]
        struct UserCommandArgs {}

        struct UserCommand {
            executed: std::sync::Arc<std::sync::atomic::AtomicBool>,
        }

        #[async_trait]
        impl Command<UserCommandArgs> for UserCommand {
            fn required_permissions(&self) -> PermissionFlags {
                PermissionFlags::USER
            }
            async fn execute(&self, _args: UserCommandArgs, _command: &CommandRequest) {
                self.executed.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let executed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (broadcaster, _messages) = mock_broadcaster();
        let (game_event_pub, _events) = mock_game_event_publisher();
        let mut subscriber = CommandSubscriber::new(broadcaster, game_event_pub);
        subscriber.register::<UserCommand, UserCommandArgs>(UserCommand {
            executed: executed.clone(),
        });

        let actor = mock_actor_elevated("AdminUser", true); 
        let req = mock_command_request(actor, "user_command", vec![]);
        let event = GameEvent::CommandRequestEvent(req);
        subscriber.on_event(&event).await;

        assert!(executed.load(std::sync::atomic::Ordering::SeqCst), "Admin should be allowed to execute USER command");
    }

    #[tokio::test]
    async fn test_command_subscriber_help() {
        let (broadcaster, messages) = mock_broadcaster();
        let (game_event_pub, _events) = mock_game_event_publisher();
        let mut subscriber = CommandSubscriber::new(broadcaster, game_event_pub);
        
        subscriber.register::<TestCommand, TestArgs>(TestCommand);
        
        // Add a second command to test aggregation
        #[derive(Parser, Debug)]
        #[command(name = "test2", about = "Another test command")]
        struct Test2Args {
            arg: String,
        }
        struct Test2Command;
        #[async_trait]
        impl Command<Test2Args> for Test2Command {
            fn required_permissions(&self) -> PermissionFlags {
                PermissionFlags::empty()
            }

            async fn execute(&self, _args: Test2Args, command: &CommandRequest) {}
        }
        subscriber.register::<Test2Command, Test2Args>(Test2Command);
        
        let actor = mock_actor("TestUser");
        let req = mock_command_request(actor, "help", vec![]);
        let event = GameEvent::CommandRequestEvent(req);
        subscriber.on_event(&event).await;
        
        // Wait a bit for the message to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let msgs = messages.lock().await;
        assert_eq!(msgs.len(), 1);
        
        // Check if the help message contains the command names
        let help_msg = &msgs[0];
        let help_text = format!("{:?}", help_msg);
        
        println!("Received help text: {}", help_text);

        assert!(help_msg.title.as_ref().unwrap().contains("🛠️ Sleuth System Help"));
        assert!(help_text.contains("`!test <message> <flag>`"));
        assert!(help_text.contains("A test command using clap"));
        assert!(help_text.contains("`!test2 <arg>`"));
        assert!(help_text.contains("Another test command"));
    }
}
