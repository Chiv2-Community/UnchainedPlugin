# Contributing to Unchained Plugin

Thank you for your interest in contributing! This guide will help you understand how to extend the plugin with new commands, vote types, and event subscribers.

## Creating a New Command

The command system uses a trait-based approach integrated with `clap` for argument parsing.

### 1. Define Command Arguments
Create a struct for your command's arguments and derive `Parser` and `CommandFactory`.

```rust
use clap::{Parser, CommandFactory};

#[derive(Parser, CommandFactory, Clone, Debug)]
#[command(name = "mycommand", about = "Description of what my command does")]
pub struct MyCommandArgs {
    #[arg(help = "A required argument")]
    pub target: String,
}
```

### 2. Implement the `Command` Trait
Implement the `Command<MyCommandArgs>` trait for your command struct.

```rust
use async_trait::async_trait;
use crate::commands::{Command, CommandResult};
use crate::events::models::{CommandRequest, PermissionFlags};

pub struct MyCommand;

#[async_trait]
impl Command<MyCommandArgs> for MyCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::all() // Or specific flags
    }

    async fn execute(&self, args: MyCommandArgs, req: &CommandRequest) {
        // Your logic here
        println!("Executing mycommand with target: {}", args.target);
    }
}
```

### 3. Register the Command
Commands must be registered to the `CommandSubscriber`.

- **Always Active**: Register in `src/features/events.rs` within `initialize_subscribers`.
- **Discord Only**: Register in `src/features/discord.rs` (if applicable).

Example in `src/features/events.rs`:
```rust
command_subscriber.register(MyCommand);
```

---

## Creating a New Vote Type

The voting system is similar to the command system but uses the `VoteType` trait.

### 1. Define Vote Arguments
Define a struct for the vote's arguments using `clap`.

```rust
#[derive(Parser, CommandFactory, Clone, Debug)]
#[command(name = "myvote")]
pub struct MyVoteArgs {
    pub value: i32,
}
```

### 2. Implement the `VoteType` Trait
Implement `VoteType<MyVoteArgs>` for your vote logic.

```rust
use crate::modules::vote::VoteType;

pub struct MyVote;

impl VoteType<MyVoteArgs> for MyVote {
    fn title(&self) -> String { "My Vote".to_string() }
    fn description(&self) -> String { "Vote for something".to_string() }
    fn vote_description(&self, args: MyVoteArgs) -> String {
        format!("Should we set the value to {}?", args.value)
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.5 }
    fn min_votes_required_ratio(&self) -> f32 { 0.2 }
    fn on_success(&self, args: MyVoteArgs) {
        // Success logic
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}
```

### 3. Register the Vote Type
Register your vote type to the `VoteCommand` in `src/features/events.rs` within `initalize_vote_commands`.

```rust
vote_command.register(MyVote).await;
```

---

## Creating a New Subscriber

Subscribers listen for events on an `EventBus`.

### 1. Implement the `Subscriber` Trait
Implement `Subscriber<T>` for the event type you are interested in (e.g., `GameEvent` or `BroadcastMessage`).

```rust
use async_trait::async_trait;
use crate::events::bus::Subscriber;

pub struct MySubscriber;

#[async_trait]
impl Subscriber<GameEvent> for MySubscriber {
    fn identifier(&self) -> &'static str { "MySubscriber" }
    
    async fn on_event(&mut self, event: &GameEvent) {
        // Handle the event
    }

    async fn on_tick(&mut self) {
        // Optional: periodic logic
    }
}
```

### 2. Register the Subscriber
Register the subscriber to the appropriate bus in `src/features/events.rs`.

```rust
let _ = game_event_bus.subscribe(Box::new(MySubscriber)).await;
```

## Architecture and State Management

### Global Statics vs. Dependency Injection

This project operates by injecting a DLL into Chivalry 2. Because we do not have full control over the game's methods or their lifecycle, some **globally accessible mutable static values** are necessary to share state between DLL hooks.

However, **you should avoid adding new global statics** whenever possible. 

- **Global Statics**: Should only be used if the state *must* be accessible inside a low-level DLL Hook.
- **Dependency Injection**: For all other components (Commands, Votes, Subscribers), you should inject the dependencies they need (such as event publishers or shared state) into their structs during initialization.

### Example: Injecting Dependencies

When creating a new component, pass its dependencies through its constructor:

```rust
pub struct MySubscriber {
    broadcaster: &'static EventPublisher<BroadcastMessage>,
    shared_data: Arc<Mutex<MyData>>,
}

impl MySubscriber {
    pub fn new(
        broadcaster: &'static EventPublisher<BroadcastMessage>, 
        shared_data: Arc<Mutex<MyData>>
    ) -> Self {
        Self { broadcaster, shared_data }
    }
}
```

Then, initialize and register it in `src/features/events.rs`:

```rust
let my_data = Arc::new(Mutex::new(MyData::new()));
let my_subscriber = MySubscriber::new(broadcast_message_publisher, my_data);
let _ = game_event_bus.subscribe(Box::new(my_subscriber)).await;
```

---

## Logs Location

If you need to debug or check the plugin's output, you can find the logs in the following directory:

`%localappdata%/Chivalry 2/Saved_{saveddirsuffix}/Logs`

Replace `{saveddirsuffix}` with the appropriate suffix for your installation (e.g., `Steam` or `Epic`).
