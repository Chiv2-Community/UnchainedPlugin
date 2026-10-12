use std::sync::Arc;
use tokio::sync::Mutex;
use async_trait::async_trait;
use clap::Parser;
use crate::commands::Command;
use crate::events::models::{
    CommandActor, ActorIdentity, ActorPermissions, PermissionFlags, 
    CommandRequest, CommandSource, GameEvent
};
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::EventPublisher;

#[derive(Parser, Debug, Clone)]
#[command(name = "test", about = "A test command using clap")]
pub struct TestArgs {
    /// A test argument (positional)
    pub message: String,
    /// A flag (positional)
    pub flag: String,
}

pub struct TestCommand;

#[async_trait]
impl Command<TestArgs> for TestCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::USER | PermissionFlags::START_VOTE
    }

    async fn execute(&self, args: TestArgs, _command: &CommandRequest) {
        println!("Test command executed with message: {} and flag: {}", args.message, args.flag);
    }
}

pub fn mock_actor(name: &str) -> CommandActor {
    mock_actor_elevated(name, true)
}

pub fn mock_actor_elevated(name: &str, elevated: bool) -> CommandActor {
    CommandActor {
        display_name: name.to_string(),
        identity: ActorIdentity::GamePlayer {
            player_id: 123,
            display_name: name.to_string(),
        },
        permissions: ActorPermissions {
            flags: if elevated {
                PermissionFlags::USER | PermissionFlags::START_VOTE | PermissionFlags::ADMIN | PermissionFlags::MODERATOR
            } else {
                PermissionFlags::USER
            },
        },
    }
}

pub fn mock_broadcaster() -> (&'static EventPublisher<BroadcastMessage>, Arc<Mutex<Vec<BroadcastMessage>>>) {
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

pub fn mock_game_event_publisher() -> (&'static EventPublisher<GameEvent>, Arc<Mutex<Vec<GameEvent>>>) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            events_clone.lock().await.push(msg);
        }
    });

    (Box::leak(Box::new(EventPublisher::new(tx))), events)
}

pub fn mock_command_request(actor: CommandActor, name: &str, args: Vec<String>) -> CommandRequest {
    CommandRequest {
        name: name.to_string(),
        raw_args: args.join(" "),
        args,
        actor,
        source: CommandSource::GameChat,
    }
}
