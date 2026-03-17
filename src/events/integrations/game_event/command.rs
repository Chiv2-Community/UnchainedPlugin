use async_trait::async_trait;
use clap::{Parser, CommandFactory};
use crate::events::bus::{Subscriber, EventPublisher};
use crate::events::models::{GameEvent, GameCommand};
use crate::events::broadcast::BroadcastMessage;

use std::marker::PhantomData;
use crate::events::integrations::command::{Command, CommandHandler, ErasedCommand};

pub struct CommandSubscriber {
    commands: Vec<Box<dyn ErasedCommand>>,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl CommandSubscriber {
    pub fn new(broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self {
            commands: Vec::new(),
            broadcaster,
        }
    }

    pub fn register<T, Args>(&mut self, command: T)
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
        }

        if let GameEvent::GameCommandEvent(cmd) = event {
            if cmd.name == "help" {
                let mut help_msg = BroadcastMessage::new()
                    .title("🛠️ Sleuth System Help".to_string())
                    .content("All available commands:".to_string())
                    .color(0x3498db)
                    .footer("💬-Discord Only | 🎮-Game Only | 🔒-Admin only".to_string());

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
                return;
            } else {
                for command in &self.commands {
                    if command.name() == cmd.name {
                        if cmd.actor.permissions.flags.intersects(command.required_permissions()) {
                            self.broadcaster.publish(format!("User '{}' executed command '{}'.", cmd.actor.display_name, cmd.name).into());
                            command.execute(cmd).await;
                        } else {
                            self
                                .broadcaster
                                .publish(format!("Error: User '{}' is not allowed to execute command '{}'.", cmd.actor.display_name, cmd.name).into());
                        }
                        return;
                    }
                }

                self.broadcaster.publish(format!("User '{}' attempted to execute non-existent command '{}'.", cmd.actor.display_name, cmd.name).into());
            }
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

    #[derive(Parser, Debug)]
    #[command(name = "test", about = "A test command using clap")]
    struct TestArgs {
        message: String,
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

    fn mock_broadcaster() -> (&'static EventPublisher<BroadcastMessage>, Arc<Mutex<Vec<BroadcastMessage>>>) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_clone = Arc::clone(&messages);

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                messages_clone.lock().await.push(msg);
            }
        });

        (Box::leak(Box::new(EventPublisher::new(tx))), messages)
    }

    #[tokio::test]
    async fn test_command_subscriber_unknown_command() {
        let (broadcaster, _messages) = mock_broadcaster();
        let mut subscriber = CommandSubscriber::new(broadcaster);
        subscriber.register::<TestCommand, TestArgs>(TestCommand);
        
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
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::USER | PermissionFlags::START_VOTE
    }

        async fn execute(&self, _args: SetBoolArgs, _command: &GameCommand) {
            self.executed.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn test_command_subscriber_multiple_commands() {
        let (broadcaster, _messages) = mock_broadcaster();
        let mut subscriber = CommandSubscriber::new(broadcaster);
        let setbool_executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        
        subscriber.register::<TestCommand, TestArgs>(TestCommand);
        subscriber.register::<SetBoolCommand, SetBoolArgs>(SetBoolCommand {
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
            ticked: Arc<std::sync::atomic::AtomicBool>,
        }
        #[async_trait]
        impl Command<TickArgs> for TickCommand {
            fn required_permissions(&self) -> PermissionFlags {
                PermissionFlags::USER | PermissionFlags::START_VOTE
            }

            async fn execute(&self, _args: TickArgs, _command: &GameCommand) {}
            async fn on_tick(&self) {
                self.ticked.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let ticked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (broadcaster, _messages) = mock_broadcaster();
        let mut subscriber = CommandSubscriber::new(broadcaster);
        subscriber.register::<TickCommand, TickArgs>(TickCommand { ticked: ticked.clone() });

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
            fn required_permissions(&self) -> PermissionFlags {
                PermissionFlags::ADMIN
            }
            async fn execute(&self, _args: RestrictedArgs, _command: &GameCommand) {
                self.executed.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let executed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (broadcaster, _messages) = mock_broadcaster();
        let mut subscriber = CommandSubscriber::new(broadcaster);
        subscriber.register::<RestrictedCommand, RestrictedArgs>(RestrictedCommand {
            executed: executed.clone(),
        });

        // Mock actor with only USER permissions
        let actor = mock_actor("NormalUser"); 
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

    #[tokio::test]
    async fn test_command_permission_flags_denied() {
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
            async fn execute(&self, _args: AdminOnlyArgs, _command: &GameCommand) {
                self.executed.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let executed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (broadcaster, _messages) = mock_broadcaster();
        let mut subscriber = CommandSubscriber::new(broadcaster);
        subscriber.register::<AdminOnlyCommand, AdminOnlyArgs>(AdminOnlyCommand {
            executed: executed.clone(),
        });

        // Mock actor with only USER permissions
        let actor = mock_actor("NormalUser"); 
        let game_cmd = GameCommand {
            name: "admin_only".to_string(),
            args: vec![],
            raw_args: "".to_string(),
            actor,
            source: CommandSource::GameChat,
        };

        let event = GameEvent::GameCommandEvent(game_cmd);
        subscriber.on_event(&event).await;

        assert!(!executed.load(std::sync::atomic::Ordering::SeqCst), "Command should not have executed without required permissions");
    }

    #[tokio::test]
    async fn test_command_subscriber_help() {
        let (broadcaster, messages) = mock_broadcaster();
        let mut subscriber = CommandSubscriber::new(broadcaster);
        
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

            async fn execute(&self, _args: Test2Args, _command: &GameCommand) {}
        }
        subscriber.register::<Test2Command, Test2Args>(Test2Command);
        
        let actor = mock_actor("TestUser");
        let game_cmd = GameCommand {
            name: "help".to_string(),
            args: vec![],
            raw_args: "".to_string(),
            actor,
            source: CommandSource::GameChat,
        };
        
        let event = GameEvent::GameCommandEvent(game_cmd);
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
