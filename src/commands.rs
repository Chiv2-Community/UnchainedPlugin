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

/// Where one console (stdin) line goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleRoute {
    /// Nothing to do (empty line, or a server line that cannot be tokenized).
    Ignore,
    /// Forward the line verbatim to the native engine console.
    Native(String),
    /// Publish a plugin command request.
    Command { name: String, args: Vec<String> },
}

/// Decides what to do with one console line. Pure logic.
///
/// `is_plugin_command` gets the lowercased first token and tells whether it
/// names a registered, console-eligible plugin command. On a client every line
/// that is not `help` or a plugin command goes to the engine console verbatim,
/// including lines shlex cannot tokenize (`say it's me`).
#[cfg_attr(not(feature = "cli_commands"), allow(dead_code))]
pub fn route_console_line(
    line: &str,
    is_server: bool,
    is_plugin_command: impl Fn(&str) -> bool,
) -> ConsoleRoute {
    let line = line.trim();
    if line.is_empty() {
        return ConsoleRoute::Ignore;
    }
    let Some(parts) = shlex::split(line) else {
        return if is_server { ConsoleRoute::Ignore } else { ConsoleRoute::Native(line.to_string()) };
    };
    let Some(first) = parts.first() else {
        return ConsoleRoute::Ignore;
    };
    let name = first.to_lowercase();
    if name == "help" {
        ConsoleRoute::Command { name: "help".to_string(), args: parts[1..].to_vec() }
    } else if is_plugin_command(&name) {
        ConsoleRoute::Command { name: first.clone(), args: parts[1..].to_vec() }
    } else if is_server {
        // Server: unrecognized lines go to the engine's `cmd`.
        ConsoleRoute::Command { name: "cmd".to_string(), args: parts }
    } else {
        ConsoleRoute::Native(line.to_string())
    }
}

#[cfg(feature="cli_commands")]
pub fn spawn_cli_handler() {
    std::thread::spawn(move || {
        let input_source = std::io::stdin();
        let mut buffer = String::new();

        while input_source.read_line(&mut buffer).is_ok_and(|count| count != 0) {
            // A line names a plugin command when it matches a registered
            // command that is console-eligible (ServerConsole or
            // source-agnostic). This check runs on every instance so the
            // console works for both dedicated servers and clients.
            let is_plugin_command = |name: &str| {
                EVENT_SYSTEM.command_subscriber.blocking_lock()
                    .get_registered_commands()
                    .iter()
                    .any(|registered_command|
                        registered_command.name().to_lowercase() == name
                            && (registered_command.required_source() == Some(CommandSource::ServerConsole)
                                || registered_command.required_source() == None
                            )
                    )
            };

            match route_console_line(&buffer, crate::cli_args().is_server(), is_plugin_command) {
                ConsoleRoute::Ignore => {}
                ConsoleRoute::Native(line) => NATIVE_COMMAND_QUEUE.lock().unwrap().push(line),
                ConsoleRoute::Command { name, args } => {
                    EVENT_SYSTEM.game_event_publisher.publish(GameEvent::CommandRequestEvent(CommandRequest {
                        name,
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
    fn group(&self) -> String {
        "General".to_string()
    }
    fn required_permissions(&self) -> PermissionFlags;
    fn required_source(&self) -> Option<CommandSource> {
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
    fn group(&self) -> String;
    fn required_source(&self) -> Option<CommandSource>;
    fn required_permissions(&self) -> PermissionFlags;
    fn help(&self, show_emojis: bool, show_key: bool) -> String;
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

    fn group(&self) -> String {
        self.command.group()
    }

    fn required_source(&self) -> Option<CommandSource> {
        self.command.required_source()
    }

    fn required_permissions(&self) -> PermissionFlags {
        self.command.required_permissions()
    }

    fn help(&self, show_emojis: bool, show_key: bool) -> String {
        use crate::events::models::{CommandSource, PermissionFlags};

        let source_tag =
            if show_key {
                if show_emojis {
                    match self.required_source() {
                        Some(CommandSource::GameChat) => "🎮",
                        Some(CommandSource::Discord) => "💬",
                        Some(CommandSource::ServerConsole) => "🖳",
                        None => "🌐",
                    }
                } else {
                    match self.required_source() {
                        Some(CommandSource::GameChat) => "[Game]",
                        Some(CommandSource::Discord) => "[Discord]",
                        Some(CommandSource::ServerConsole) => "[Console]",
                        None => "[Global]",
                    }
                }
            } else {
                ""
            };

        let lock = if self.required_permissions().contains(PermissionFlags::ADMIN) { 
            if show_emojis { "🔒" } else { "[Admin]" }
        } else { 
            "" 
        };

        format!("`{}` - {}{} *{}*", self.usage(), source_tag, lock, self.description())
    }

    async fn execute(&self, command: &CommandRequest) {
        let mut args = vec![command.name.clone()];
        args.extend(command.args.clone());

        match Args::try_parse_from(args) {
            Ok(parsed) => {
                self.command.execute(parsed, command).await;
            }
            Err(e) => {
                swarn!("Failed to parse command arguments for {}: {}", command.name, e);
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

    fn command(name: &str, args: &[&str]) -> ConsoleRoute {
        ConsoleRoute::Command {
            name: name.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }

    #[test]
    fn client_lines_shlex_cannot_parse_go_to_the_native_console() {
        let known = |name: &str| name == "mod-list";
        for line in ["say it's me\n", "open \"Some Map", "exec foo\\"] {
            assert_eq!(
                route_console_line(line, false, known),
                ConsoleRoute::Native(line.trim().to_string())
            );
        }
        // Servers never forwarded these lines; that is unchanged.
        assert_eq!(route_console_line("say it's me", true, known), ConsoleRoute::Ignore);
    }

    #[test]
    fn console_lines_are_routed_by_instance_and_command() {
        let known = |name: &str| name == "mod-list";
        assert_eq!(route_console_line("  \r\n", false, known), ConsoleRoute::Ignore);
        assert_eq!(route_console_line("", true, known), ConsoleRoute::Ignore);
        assert_eq!(route_console_line("Mod-List -v\n", false, known), command("Mod-List", &["-v"]));
        assert_eq!(route_console_line("HELP mod-list", true, known), command("help", &["mod-list"]));
        assert_eq!(
            route_console_line("stat fps\n", false, known),
            ConsoleRoute::Native("stat fps".to_string())
        );
        assert_eq!(route_console_line("stat fps", true, known), command("cmd", &["stat", "fps"]));
    }

}
