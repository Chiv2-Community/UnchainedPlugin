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
use crate::events::models::{CommandExecuted, CommandRejected, CommandRequest, GameEvent, CommandSource};
use crate::events::broadcast::BroadcastMessage;
// use crate::features::events::EVENT_SYSTEM;

// use std::marker::PhantomData;
use crate::commands::{Command, CommandHandler, ErasedCommand};
use crate::sinfo;
use crate::features::discord::{SharedDiscordConfig, send_to_channel};
use std::sync::Arc;
use serenity::all::Http;

pub struct CommandSubscriber {
    commands: Vec<Box<dyn ErasedCommand>>,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
    game_event_publisher: &'static EventPublisher<GameEvent>,
    discord_config: Option<SharedDiscordConfig>,
    discord_http: Option<Arc<Http>>,
}

enum CommandOutcome {
    Executed,
    Rejected(CommandRejected),
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
            discord_config: None,
            discord_http: None,
        }
    }

    pub fn set_discord_connection(&mut self, config: SharedDiscordConfig, http: Arc<Http>) {
        self.discord_config = Some(config);
        self.discord_http = Some(http);
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

    async fn send_help(&self, source: CommandSource, show_all: bool) {
        let show_emojis = source == CommandSource::Discord;
        
        let title = if show_emojis { "🛠️ Sleuth System Help" } else { "Sleuth System Help" };
        let content = if show_all { 
            "All available commands:".to_string() 
        } else { 
            format!("Available commands for {}:", source) 
        };

        let mut field_text = String::new();
        let mut seen_sources = std::collections::HashSet::new();

        for command in &self.commands {
            let req_source = command.required_source();
            
            if !show_all {
                if let Some(ref rs) = req_source {
                    if *rs != source {
                        continue;
                    }
                }
            }

            if !field_text.is_empty() {
                field_text.push('\n');
            }
            field_text.push_str(&command.help(show_emojis, show_all));
            
            if let Some(rs) = req_source {
                seen_sources.insert(Some(rs));
            } else {
                seen_sources.insert(None);
            }
        }

        let mut footer = String::new();
        let mut footer_parts = Vec::new();

        if show_all {
            if seen_sources.contains(&Some(CommandSource::Discord)) {
                footer_parts.push(if show_emojis { "💬-Discord Only" } else { "[Discord]-Discord Only" });
            }
            if seen_sources.contains(&Some(CommandSource::GameChat)) {
                footer_parts.push(if show_emojis { "🎮-Game Only" } else { "[Game]-Game Only" });
            }
            if seen_sources.contains(&Some(CommandSource::ServerConsole)) {
                footer_parts.push(if show_emojis { "🖳-Server Console Only" } else { "[Console]-Server Console Only" });
            }
            if seen_sources.contains(&None) {
                footer_parts.push(if show_emojis { "🌐-Executable from anywhere" } else { "[Global]-Executable from anywhere" });
            }

            footer_parts.push(if show_emojis { "🔒-Admin only" } else { "[Admin]-Admin only" });
        }
        
        footer.push_str(&footer_parts.join(" | "));

        if field_text.is_empty() {
            return;
        }

        match source {
            CommandSource::Discord => {
                if let (Some(config), Some(http)) = (&self.discord_config, &self.discord_http) {
                    let help_msg = serenity::all::CreateMessage::new()
                        .embed(serenity::all::CreateEmbed::new()
                            .title(title)
                            .description(content)
                            .field("Commands", field_text, false)
                            .footer(serenity::all::CreateEmbedFooter::new(footer))
                            .color(0x3498db)
                        );
                    
                    let channel_id = config.read().await.general_chat_channel_id;
                    send_to_channel(http, channel_id, help_msg).await;
                }
            }
            CommandSource::GameChat => {
                let full_msg = format!("{}: {}\n{}\n{}", title, content, field_text, footer);
                #[cfg(not(test))]
                crate::game::chivalry2::send_ingame_message(full_msg, Some(crate::game::chivalry2::EChatType::ServerSay));
                #[cfg(test)]
                self.broadcaster.publish(full_msg.into());
            }
            CommandSource::ServerConsole => {
                crate::sinfo!("{}: {}\n{}\n{}", title, content, field_text, footer);
            }
        }
    }

    async fn handle_command(&self, command: &dyn ErasedCommand, req: &CommandRequest) -> CommandOutcome {
        let executed_from_console = req.source == CommandSource::ServerConsole;

        if let Some(required_source) = command.required_source() {
            if required_source != req.source {
                let rejection_reason = format!(
                    "Command '{}' must be executed from {}, but request source was {}",
                    req.name, required_source, req.source
                );

                if !executed_from_console {
                    self
                        .broadcaster
                        .publish(format!("Error: User '{}' attempted to execute command '{}' from {}, but it must be executed from {}", req.actor.display_name, req.name, req.source, required_source).into());
                }
                return CommandOutcome::Rejected(CommandRejected {
                    name: req.name.clone(),
                    args: req.args.clone(),
                    raw_args: req.raw_args.clone(),
                    actor: req.actor.clone(),
                    source: req.source.clone(),
                    rejection_reason,
                });
            }
        }

        if !req.actor.permissions.flags.contains(command.required_permissions()) {
            let rejection_reason = format!(
                "User '{}' is missing required permissions to execute '{}'",
                req.actor.display_name, req.name
            );

            if !executed_from_console {
                self
                    .broadcaster
                    .publish(format!("Error: User '{}' is not allowed to execute command '{}'.", req.actor.display_name, req.name).into());
            }
            return CommandOutcome::Rejected(CommandRejected {
                name: req.name.clone(),
                args: req.args.clone(),
                raw_args: req.raw_args.clone(),
                actor: req.actor.clone(),
                source: req.source.clone(),
                rejection_reason,
            });
        }

        if !executed_from_console {
            sinfo!("User '{}' executed command '{} {}'", req.actor.display_name, req.name, req.raw_args);
        }

        command.execute(req).await;
        CommandOutcome::Executed
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
                let show_all = req.args.iter().any(|arg| arg == "--all");
                self.send_help(req.source.clone(), show_all).await;
                return;
            }

            let maybe_command =
                self.commands.iter().find(|c| c.name() == req.name);

            if maybe_command.is_none() {
                let rejection_reason = format!("Command '{}' does not exist", req.name);
                self.broadcaster.publish(format!("User '{}' attempted to execute non-existent command '{}'.", req.actor.display_name, req.name).into());
                let rejected = CommandRejected {
                    name: req.name.clone(),
                    args: req.args.clone(),
                    raw_args: req.raw_args.clone(),
                    actor: req.actor.clone(),
                    source: req.source.clone(),
                    rejection_reason,
                };
                self.game_event_publisher.publish(GameEvent::CommandRejectedEvent(rejected));
                return;
            }

            match self.handle_command(maybe_command.unwrap().as_ref(), req).await {
                CommandOutcome::Executed => {
                    let cmd = CommandExecuted {
                        name: req.name.clone(),
                        args: req.args.clone(),
                        raw_args: req.raw_args.clone(),
                        actor: req.actor.clone(),
                        source: req.source.clone(),
                    };
                    self.game_event_publisher.publish(GameEvent::CommandExecutedEvent(cmd));
                }
                CommandOutcome::Rejected(rejected) => {
                    self.game_event_publisher.publish(GameEvent::CommandRejectedEvent(rejected));
                }
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
    use crate::events::models::PermissionFlags;
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
        pub executed: Arc<std::sync::atomic::AtomicBool>,
    }

    #[async_trait]
    impl Command<SetBoolArgs> for SetBoolCommand {
        fn required_permissions(&self) -> PermissionFlags {
            PermissionFlags::START_VOTE
        }

        async fn execute(&self, _args: SetBoolArgs, _command: &CommandRequest) {
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

            async fn execute(&self, _args: TickArgs, _command: &CommandRequest) {}
            async fn on_tick(&self) {
                self.ticked.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let ticked = Arc::new(std::sync::atomic::AtomicBool::new(false));
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
            executed: Arc<std::sync::atomic::AtomicBool>,
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

        let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
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
            executed: Arc<std::sync::atomic::AtomicBool>,
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
    async fn test_help_from_game_chat() {
        let (broadcaster, messages) = mock_broadcaster();
        let (game_event_pub, _events) = mock_game_event_publisher();
        let mut subscriber = CommandSubscriber::new(broadcaster, game_event_pub);
        
        subscriber.register::<TestCommand, TestArgs>(TestCommand);
        
        #[derive(Parser, Debug)]
        #[command(name = "discord_only", about = "Discord only command")]
        struct DiscordOnlyArgs {}
        struct DiscordOnlyCommand;
        #[async_trait]
        impl Command<DiscordOnlyArgs> for DiscordOnlyCommand {
            fn required_permissions(&self) -> PermissionFlags {
                PermissionFlags::empty()
            }
            fn required_source(&self) -> Option<crate::events::models::CommandSource> {
                Some(crate::events::models::CommandSource::Discord)
            }
            async fn execute(&self, _args: DiscordOnlyArgs, _command: &CommandRequest) {}
        }
        subscriber.register::<DiscordOnlyCommand, DiscordOnlyArgs>(DiscordOnlyCommand);
        
        let actor = mock_actor("TestUser");
        let mut req = mock_command_request(actor, "help", vec![]);
        req.source = crate::events::models::CommandSource::GameChat;
        
        let event = GameEvent::CommandRequestEvent(req);
        subscriber.on_event(&event).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        {
            let msgs = messages.lock().await;
            let help_text = format!("{:?}", msgs.last().unwrap());
            assert!(help_text.contains("`!test <message> <flag>`"));
            assert!(!help_text.contains("`!discord_only`"));
            assert!(!help_text.contains("[Game]-Game Only"));
            assert!(!help_text.contains("[Global]-Executable from anywhere"));
            assert!(!help_text.contains("🎮"));
            assert!(!help_text.contains("🌐"));
        }
    }

    #[tokio::test]
    async fn test_help_from_discord() {
        let (broadcaster, messages) = mock_broadcaster();
        let (game_event_pub, _events) = mock_game_event_publisher();
        let mut subscriber = CommandSubscriber::new(broadcaster, game_event_pub);
        
        subscriber.register::<TestCommand, TestArgs>(TestCommand);
        
        let actor = mock_actor("TestUser");
        let mut req_discord = mock_command_request(actor, "help", vec![]);
        req_discord.source = crate::events::models::CommandSource::Discord;
        subscriber.on_event(&GameEvent::CommandRequestEvent(req_discord)).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        {
            let msgs = messages.lock().await;
            // No message sent to broadcaster since source was Discord and no discord connection set
            assert_eq!(msgs.len(), 0); 
        }
    }

    #[tokio::test]
    async fn test_help_all_from_game_chat() {
        let (broadcaster, messages) = mock_broadcaster();
        let (game_event_pub, _events) = mock_game_event_publisher();
        let mut subscriber = CommandSubscriber::new(broadcaster, game_event_pub);
        
        subscriber.register::<TestCommand, TestArgs>(TestCommand);
        
        #[derive(Parser, Debug)]
        #[command(name = "discord_only", about = "Discord only command")]
        struct DiscordOnlyArgs {}
        struct DiscordOnlyCommand;
        #[async_trait]
        impl Command<DiscordOnlyArgs> for DiscordOnlyCommand {
            fn required_permissions(&self) -> PermissionFlags {
                PermissionFlags::empty()
            }
            fn required_source(&self) -> Option<crate::events::models::CommandSource> {
                Some(crate::events::models::CommandSource::Discord)
            }
            async fn execute(&self, _args: DiscordOnlyArgs, _command: &CommandRequest) {}
        }
        subscriber.register::<DiscordOnlyCommand, DiscordOnlyArgs>(DiscordOnlyCommand);
        
        let actor = mock_actor("TestUser");
        let mut req_all = mock_command_request(actor, "help", vec!["--all".to_string()]);
        req_all.source = crate::events::models::CommandSource::GameChat;
        subscriber.on_event(&GameEvent::CommandRequestEvent(req_all)).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        {
            let msgs = messages.lock().await;
            let help_text = format!("{:?}", msgs.last().unwrap());
            assert!(help_text.contains("`!test <message> <flag>`"));
            assert!(help_text.contains("`!discord_only`"));
            assert!(help_text.contains("[Discord]-Discord Only"));
            assert!(help_text.contains("[Global]-Executable from anywhere"));
        }
    }
}
