use async_trait::async_trait;
use clap::Parser;
use crate::events::bus::Subscriber;
use crate::events::models::{GameEvent, GameCommand};

use std::marker::PhantomData;

#[async_trait]
pub trait Command<Args: Parser + Send + Sync>: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    async fn execute(&self, args: Args, command: &GameCommand);
}

#[async_trait]
pub trait ErasedCommand: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    async fn execute(&self, command: &GameCommand);
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
    Args: Parser + Send + Sync,
{
    fn name(&self) -> &'static str {
        self.command.name()
    }

    fn description(&self) -> &'static str {
        self.command.description()
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
}

pub struct CommandSubscriber {
    commands: Vec<Box<dyn ErasedCommand>>,
}

impl CommandSubscriber {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn register_command<T, Args>(&mut self, command: T)
    where
        T: Command<Args> + 'static,
        Args: Parser + Send + Sync + 'static,
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
        if let GameEvent::GameCommandEvent(cmd) = event {
            for command in &self.commands {
                if command.name() == cmd.name {
                    command.execute(cmd).await;
                }
            }
        }
    }
}

/// Sample command using clap for parsing
#[derive(Parser, Debug)]
#[command(name = "sample", about = "A sample command using clap")]
struct SampleArgs {
    /// A sample argument
    #[arg(short, long)]
    message: String,

    /// An optional flag
    #[arg(short, long)]
    flag: bool,
}

pub struct SampleCommand;

#[async_trait]
impl Command<SampleArgs> for SampleCommand {
    fn name(&self) -> &'static str {
        "sample"
    }

    fn description(&self) -> &'static str {
        "A sample command that demonstrates clap integration"
    }

    async fn execute(&self, args: SampleArgs, _command: &GameCommand) {
        println!("Sample command executed with message: {} and flag: {}", args.message, args.flag);
        // In a real scenario, you might want to send a response back or trigger other actions
    }
}
