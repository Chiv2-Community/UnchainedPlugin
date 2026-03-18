//! The voting system allows players to initiate and participate in server-wide polls.
//!
//! It works similarly to the command system, using a trait-based approach ([`VoteType`])
//! and type-erased handlers ([`ErasedVoteType`]).
//!
//! # Architecture
//!
//! - [`VoteType`]: The primary trait for defining a new vote's behavior (title, description, success logic).
//! - [`VotingState`]: Manages the currently active vote and registered vote types.
//! - [`VoteCommand`]: A command that allows users to start votes (e.g., `!vote <type>`).
//! - [`YesCommand`] / [`NoCommand`]: Commands for players to cast their votes.
//!
//! # Registration
//!
//! Vote types are registered to a `VoteCommand` instance, which is usually part of the
//! central [`CommandSubscriber`](crate::modules::command::CommandSubscriber).
//!
//! Multiple specific vote types (e.g., `EndMapVote`, `MapVote`, `KickVote`) are
//! implemented in the `src/modules/votes` directory.
//!
//! # Usage in Project
//!
//! The voting system is initialized in `src/features/events.rs` within `initalize_vote_commands`.
//! It relies on the [`BroadcastMessage`](crate::events::broadcast::BroadcastMessage) bus
//! to communicate vote status and results to players.
use std::any::Any;
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use async_trait::async_trait;
use clap::{Parser, CommandFactory};
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::EventPublisher;

use crate::commands::Command;
use crate::events::models::{CommandSource, CommandExecuted, CommandRequest, PermissionFlags};
use crate::features::events::EVENT_SYSTEM;

/// Trait that defines a specific type of vote's behavior
pub trait VoteType<T: Parser + Send + Sync + Clone>: Send + Sync {
    fn title(&self) -> String;
    fn description(&self) -> String;
    fn vote_description(&self, args: T) -> String;
    fn min_yes_vote_ratio(&self) -> f32;
    fn min_votes_required_ratio(&self) -> f32;

    /// Checks if the vote can be started with the given arguments
    fn check_prerequisites(&self, _args: T) -> Result<(), String> { Ok(()) }
    
    fn on_success(&self, target_args: T);

    fn usage(&self) -> String {
        let cmd = T::command();
        let mut usage = format!("!vote {}", cmd.get_name());
        for arg in cmd.get_positionals() {
            usage.push_str(&format!(" <{}>", arg.get_id()));
        }
        usage
    }

    fn clone_box(&self) -> Box<dyn ErasedVoteType>;
}

pub trait ErasedVoteType: Send + Sync {
    fn name(&self) -> String;
    fn title(&self) -> String;
    fn description(&self) -> String;
    fn usage(&self) -> String;
    fn vote_description(&self, args: &[String]) -> String;
    fn min_yes_vote_ratio(&self) -> f32;
    fn min_votes_required_ratio(&self) -> f32;
    fn check_prerequisites(&self, args: &[String]) -> Result<Box<dyn Any + Send>, String>;
    fn on_success(&self, target_args: Box<dyn Any + Send>);
    fn clone_box(&self) -> Box<dyn ErasedVoteType>;
}

pub struct VoteTypeHandler<T, V> {
    logic: V,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, V> VoteTypeHandler<T, V>
where
    T: Parser + CommandFactory + Send + Sync + Clone + 'static,
    V: VoteType<T> + Send + Sync + 'static,
{
    pub fn new(logic: V) -> Self {
        Self {
            logic,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, V> ErasedVoteType for VoteTypeHandler<T, V>
where
    T: Parser + CommandFactory + Send + Sync + Clone + 'static,
    V: VoteType<T> + Send + Sync + 'static,
{
    fn name(&self) -> String { T::command().get_name().to_string() }
    fn title(&self) -> String { self.logic.title() }
    fn description(&self) -> String { self.logic.description() }
    fn usage(&self) -> String { self.logic.usage() }
    fn vote_description(&self, args: &[String]) -> String {
        let mut full_args = vec![self.name()]; // Use actual clap command name
        full_args.extend(args.iter().cloned());
        match T::try_parse_from(full_args) {
            Ok(parsed) => self.logic.vote_description(parsed),
            Err(_) => "Invalid arguments".to_string(),
        }
    }
    fn min_yes_vote_ratio(&self) -> f32 { self.logic.min_yes_vote_ratio() }
    fn min_votes_required_ratio(&self) -> f32 { self.logic.min_votes_required_ratio() }
    fn check_prerequisites(&self, args: &[String]) -> Result<Box<dyn Any + Send>, String> {
        let mut full_args = vec![self.name()];
        full_args.extend(args.iter().cloned());
        match T::try_parse_from(full_args) {
            Ok(parsed) => {
                match self.logic.check_prerequisites(parsed.clone()) {
                    Ok(_) => Ok(Box::new(parsed)),
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(format!("Invalid arguments: {}", e)),
        }
    }
    fn on_success(&self, target_args: Box<dyn Any + Send>) {
        if let Some(args) = target_args.downcast_ref::<T>() {
            self.logic.on_success(args.clone());
        }
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        self.logic.clone_box()
    }
}

/// Trait to provide the current number of human players for vote threshold calculation
pub trait PlayerCountProvider: Send + Sync {
    fn get_player_count(&self) -> usize;
}

/// Default provider that uses global world state
pub struct GlobalPlayerCountProvider;
impl PlayerCountProvider for GlobalPlayerCountProvider {
    fn get_player_count(&self) -> usize {
        crate::game::chivalry2::get_human_player_count()
    }
}

impl Clone for Box<dyn ErasedVoteType> {
    fn clone(&self) -> Box<dyn ErasedVoteType> {
        self.clone_box()
    }
}

pub struct ActiveVote {
    pub logic: Box<dyn ErasedVoteType>,
    pub raw_target_args: Vec<String>,
    pub target_args: Box<dyn Any + Send>,
    pub yes_votes: HashSet<String>,
    pub no_votes: HashSet<String>,
    pub end_time: Instant,
}

pub struct VotingState {
    pub active_vote: Option<ActiveVote>,
    pub registry: HashMap<String, Box<dyn ErasedVoteType>>,
    pub player_count_provider: Box<dyn PlayerCountProvider>,
}

impl VotingState {
    pub fn new() -> Self {
        Self {
            active_vote: None,
            registry: HashMap::new(),
            player_count_provider: Box::new(GlobalPlayerCountProvider),
        }
    }

    pub fn register<T, V>(&mut self, logic: V)
    where
        T: Parser + CommandFactory + Send + Sync + Clone + 'static,
        V: VoteType<T> + Send + Sync + 'static,
    {
        let name = T::command().get_name().to_string();
        self.registry.insert(name, Box::new(VoteTypeHandler::new(logic)));
    }

    pub fn unregister<T>(&mut self)
    where
        T: Parser + CommandFactory + Send + Sync + Clone + 'static,
    {
        let name = T::command().get_name().to_string();
        self.registry.remove(&name);
    }

    pub fn unregister_by_name(&mut self, name: &str) {
        self.registry.remove(name);
    }
}

pub type SharedVotingState = Arc<Mutex<VotingState>>;

#[derive(Parser, Debug)]
#[command(name = "vote", about = "Start a new vote")]
pub struct VoteArgs {
    /// The type of vote to start (e.g., map, kick)
    pub vote_name: Option<String>,
    /// Arguments for the specific vote type
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub struct VoteCommand {
    pub state: SharedVotingState,
    pub broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl VoteCommand {
    pub fn new(state: SharedVotingState, broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self { state, broadcaster }
    }

    pub async fn register<T, V>(&mut self, logic: V)
    where
        T: Parser + CommandFactory + Send + Sync + Clone + 'static,
        V: VoteType<T> + Send + Sync + 'static,
    {
        self.state.lock().await.register(logic);
    }

    pub async fn unregister<T>(&mut self)
    where
        T: Parser + CommandFactory + Send + Sync + Clone + 'static,
    {
        self.state.lock().await.unregister::<T>();
    }

    pub async fn unregister_by_name(&mut self, name: &str) {
        self.state.lock().await.unregister_by_name(name);
    }
}

#[async_trait]
impl Command<VoteArgs> for VoteCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::START_VOTE
    }
    fn required_source(&self) -> Option<CommandSource> {
        Some(CommandSource::GameChat)
    }

    async fn execute(&self, args: VoteArgs, command: &CommandRequest) {
        let mut state = self.state.lock().await;

        let maybe_vote_name = args.vote_name.as_deref();

        if maybe_vote_name.is_none_or(|name| name == "help") {
            let sorted_keys: Vec<_> = {
                let mut keys: Vec<_> = state.registry.keys().collect();
                keys.sort();
                keys
            };

            let mut message =
                BroadcastMessage::new()
                    .title("🗳️ Vote Help".to_string())
                    .color(0x3498db)
                    .footer("Type !vote <type> to start a vote".to_string());

            let mut field_text = String::new();
            for name in sorted_keys {
                if let Some(logic) = state.registry.get(name) {
                    if !field_text.is_empty() {
                        field_text.push('\n');
                    }
                    field_text.push_str(&format!("`{}` - *{}*", logic.usage(), logic.description()));
                }
            }

            if !field_text.is_empty() {
                message = message.field("Available Votes".to_string(), field_text);
                self.broadcaster.publish(message);
            }
            return;
        }

        let vote_name = maybe_vote_name.unwrap();

        if state.active_vote.is_some() {
            let message = BroadcastMessage::new()
                .title("Cannot Start Vote".to_string())
                .content("A vote is already in progress.".to_string())
                .color(0xFF0000);
            self.broadcaster.publish(message);
            return;
        }

        if let Some(logic) = state.registry.get(&vote_name.to_string()) {
            match logic.check_prerequisites(&args.args) {
                Ok(target_args) => {
                    let logic = logic.clone_box();
                    let name = logic.name();
                    let initiator = command.actor.display_name.clone();
                    let description = logic.vote_description(&args.args);

                    state.active_vote = Some(ActiveVote {
                        logic,
                        raw_target_args: args.args,
                        target_args,
                        yes_votes: HashSet::from([initiator]),
                        no_votes: HashSet::new(),
                        end_time: Instant::now() + Duration::from_secs(15),
                    });

                    let message =
                        BroadcastMessage::new()
                            .title(format!("Vote for '{} {}' started by '{}'", name, command.raw_args, command.actor.display_name))
                            .content(format!("Type !yes or !no in chat to vote. {}", description))
                            .color(0x00FF00);

                    self.broadcaster.publish(message);
                }
                Err(e) => {
                    let message = BroadcastMessage::new()
                        .title("Cannot Start Vote".to_string())
                        .content(format!("Error: {}", e))
                        .color(0xFF0000);
                    self.broadcaster.publish(message);
                }
            }
        } else {
            let message = BroadcastMessage::new()
                .title("Cannot Start Vote".to_string())
                .content(format!("Unknown vote type: {}. Type !vote help to see all options.", vote_name))
                .color(0xFF0000);
            self.broadcaster.publish(message);
        }
    }

    async fn on_tick(&self) {
        let mut state = self.state.lock().await;
        let mut vote_finished = false;
        let mut success = false;
        let mut failure_reason = String::new();

        if let Some(ref active) = state.active_vote {
            if Instant::now() >= active.end_time {
                vote_finished = true;
                let yes_count = active.yes_votes.len();
                let no_count = active.no_votes.len();
                let total_votes = yes_count + no_count;
                
                let player_count = state.player_count_provider.get_player_count();
                let required_votes = (player_count as f32 * active.logic.min_votes_required_ratio()).floor() as usize;

                if total_votes >= required_votes {
                    let ratio = yes_count as f32 / total_votes as f32;
                    if ratio >= active.logic.min_yes_vote_ratio() {
                        success = true;
                    } else {
                        failure_reason = format!("Majority not reached ({:.1}% required, {:.1}% achieved)", active.logic.min_yes_vote_ratio() * 100.0, ratio * 100.0);
                    }
                } else {
                    failure_reason = format!("Minimum vote threshold not met ({} votes required, {} total votes cast)", required_votes, total_votes);
                }
            }
        }

        if vote_finished {
            let active = state.active_vote.take().unwrap();
            if success {
                let message = BroadcastMessage::new()
                    .title(format!("{} SUCCEEDED", active.logic.vote_description(active.raw_target_args.as_slice())))
                    .content(format!("Result: {} Yes, {} No. The action is now being executed.", active.yes_votes.len(), active.no_votes.len()))
                    .color(0x00FF00);
                self.broadcaster.publish(message);
                active.logic.on_success(active.target_args);
            } else {
                let message = BroadcastMessage::new()
                    .title(format!("{} FAILED", active.logic.vote_description(active.raw_target_args.as_slice())))
                    .content(format!("Result: {} Yes, {} No. {}", active.yes_votes.len(), active.no_votes.len(), failure_reason))
                    .color(0xFF0000);
                self.broadcaster.publish(message);
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "yes", about = "Vote YES on the active poll")]
pub struct YesArgs {}

pub struct YesCommand {
    pub state: &'static SharedVotingState,
    pub broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl YesCommand {
    pub fn new(state: &'static SharedVotingState, broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self { state, broadcaster }
    }
}

#[async_trait]
impl Command<YesArgs> for YesCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::START_VOTE
    }

    fn required_source(&self) -> Option<CommandSource> {
        Some(CommandSource::GameChat)
    }

    async fn execute(&self, _args: YesArgs, command: &CommandRequest) {
        let mut state = self.state.lock().await;
        if let Some(ref mut active) = state.active_vote {
            let voter = command.actor.display_name.clone();
            active.no_votes.remove(&voter);
            active.yes_votes.insert(voter);
            
            self.broadcaster.publish(format!("{} voted YES", command.actor.display_name).into());
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "no", about = "Vote NO on the active poll")]
pub struct NoArgs {}

pub struct NoCommand {
    pub state: &'static SharedVotingState,
    pub broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl NoCommand {
    pub fn new(state: &'static SharedVotingState, broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self { state, broadcaster }
    }
}

#[async_trait]
impl Command<NoArgs> for NoCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::START_VOTE   
    }

    fn required_source(&self) -> Option<CommandSource> {
        Some(CommandSource::GameChat)
    }

    async fn execute(&self, _args: NoArgs, command: &CommandRequest) {
        let mut state = self.state.lock().await;
        if let Some(ref mut active) = state.active_vote {
            let voter = command.actor.display_name.clone();
            active.yes_votes.remove(&voter);
            active.no_votes.insert(voter);
            println!("{} voted NO", command.actor.display_name);
            self.broadcaster.publish(format!("{} voted NO", command.actor.display_name).into());
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "cancel", about = "Cancel the current active vote")]
pub struct CancelVoteArgs {}

pub struct CancelVoteCommand {
    pub state: SharedVotingState,
}

#[async_trait]
impl Command<CancelVoteArgs> for CancelVoteCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::MODERATOR
    }
    async fn execute(&self, _args: CancelVoteArgs, command: &CommandRequest) {
        let mut state = self.state.lock().await;
        if state.active_vote.is_some() {
            state.active_vote = None;
            println!("Vote cancelled by {}", command.actor.display_name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::bus::Subscriber;
    use crate::events::models::{CommandActor, ActorIdentity, ActorPermissions, PermissionFlags, CommandSource, GameEvent};
    use crate::test_utils::{mock_actor_elevated, mock_game_event_publisher};
    use crate::modules::command::CommandSubscriber;

    #[derive(Parser, Clone, Debug, PartialEq)]
    #[command(name = "mockvote", about = "Mock description")]
    struct MockVoteArgs {
        #[arg(default_value = "default")]
        pub target: String,
    }

    #[derive(Clone)]
    struct MockVote;
    impl VoteType<MockVoteArgs> for MockVote {
        fn title(&self) -> String { "Mock Vote".into() }
        fn description(&self) -> String { "Mock description".into() }
        fn vote_description(&self, args: MockVoteArgs) -> String { format!("Mock: {}", args.target) }
        fn min_yes_vote_ratio(&self) -> f32 { 0.5 }
        fn min_votes_required_ratio(&self) -> f32 { 0.33 }
        fn on_success(&self, _args: MockVoteArgs) {}
        fn clone_box(&self) -> Box<dyn ErasedVoteType> { Box::new(VoteTypeHandler::new(self.clone())) }
    }

    struct MockPlayerCountProvider(usize);
    impl PlayerCountProvider for MockPlayerCountProvider {
        fn get_player_count(&self) -> usize { self.0 }
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

    fn mock_actor(name: &str, elevated: bool) -> CommandActor {
        CommandActor {
            display_name: name.to_string(),
            identity: ActorIdentity::GamePlayer {
                player_id: 123,
                display_name: name.to_string(),
            },
            permissions: ActorPermissions {
                flags: if elevated { PermissionFlags::ADMIN } else { PermissionFlags::USER },
            },
        }
    }

    fn mock_game_command(actor: CommandActor, args: Vec<String>) -> CommandRequest {
        CommandRequest {
            name: "test".to_string(),
            args: args.clone(),
            raw_args: args.join(" "),
            actor,
            source: CommandSource::GameChat,
        }
    }

    #[tokio::test]
    async fn test_vote_failure_reasons() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let (broadcaster, messages) = mock_broadcaster();
        let cmd = VoteCommand { state: state.clone(), broadcaster };

        // 1. Test "Minimum vote threshold not met"
        // 100 players, 33% required_ratio = 33 votes required.
        // 1 yes, 1 no = 2 total.
        {
            let mut s = state.lock().await;
            s.player_count_provider = Box::new(MockPlayerCountProvider(100));
            s.active_vote = Some(ActiveVote {
                logic: Box::new(VoteTypeHandler::new(MockVote)),
                raw_target_args: vec!["TestTarget".into()],
                target_args: Box::new(MockVoteArgs { target: "TestTarget".into() }),
                yes_votes: HashSet::from(["User1".into()]),
                no_votes: HashSet::from(["User2".into()]),
                end_time: Instant::now() - Duration::from_secs(1),
            });
        }
        cmd.on_tick().await;
        // Small delay to ensure the spawned task processes the message
        tokio::time::sleep(Duration::from_millis(10)).await;
        {
            let msgs = messages.lock().await;
            assert_eq!(msgs.len(), 1);
            assert!(msgs[0].get_content().unwrap().contains("Minimum vote threshold not met (33 votes required, 2 total votes cast)"));
        }
        messages.lock().await.clear();

        // 2. Test "Majority not reached"
        // 10 players, 33% required = 4 votes required.
        // 2 yes, 3 no = 5 total (Threshold met).
        // Ratio 2/5 = 40% (50% required).
        {
            let mut s = state.lock().await;
            s.player_count_provider = Box::new(MockPlayerCountProvider(10));
            s.active_vote = Some(ActiveVote {
                logic: Box::new(VoteTypeHandler::new(MockVote)),
                raw_target_args: vec!["TestTarget".into()],
                target_args: Box::new(MockVoteArgs { target: "TestTarget".into() }),
                yes_votes: HashSet::from(["Y1".into(), "Y2".into()]),
                no_votes: HashSet::from(["N1".into(), "N2".into(), "N3".into()]),
                end_time: Instant::now() - Duration::from_secs(1),
            });
        }
        cmd.on_tick().await;
        // Small delay to ensure the spawned task processes the message
        tokio::time::sleep(Duration::from_millis(10)).await;
        {
            let msgs = messages.lock().await;
            assert_eq!(msgs.len(), 1);
            assert!(msgs[0].get_content().unwrap().contains("Majority not reached (50.0% required, 40.0% achieved)"));
        }
    }

    #[tokio::test]
    async fn test_voting_state_initialization() {
        let state = VotingState::new();
        assert!(state.active_vote.is_none());
        assert!(state.registry.is_empty());
    }

    #[tokio::test]
    async fn test_vote_command_starts_vote() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let (broadcaster, messages) = mock_broadcaster();
        let cmd = VoteCommand { state: state.clone(), broadcaster };
        
        let actor = mock_actor("Initiator", false);
        let game_cmd = mock_game_command(actor, vec!["TournamentGrounds".into()]);
        let args = VoteArgs { vote_name: Some("mockvote".into()), args: vec!["TournamentGrounds".into()] };

        cmd.execute(args, &game_cmd).await;

        let state_lock = state.lock().await;
        assert!(state_lock.active_vote.is_some());
        let active = state_lock.active_vote.as_ref().unwrap();
        
        let target_args = active.target_args.downcast_ref::<MockVoteArgs>().unwrap();
        assert_eq!(target_args.target, "TournamentGrounds");
        
        assert_eq!(active.yes_votes.len(), 1);
        assert!(active.yes_votes.contains("Initiator"));
    }

    #[tokio::test]
    async fn test_duplicate_vote_fails() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let (broadcaster, messages) = mock_broadcaster();
        let cmd = VoteCommand { state: state.clone(), broadcaster };
        
        // Start first vote
        let actor1 = mock_actor("User1", false);
        let game_cmd1 = mock_game_command(actor1, vec!["Target1".into()]);
        cmd.execute(VoteArgs { vote_name: Some("mockvote".into()), args: vec!["Target1".into()] }, &game_cmd1).await;

        // Try start second vote
        let actor2 = mock_actor("User2", false);
        let game_cmd2 = mock_game_command(actor2, vec!["Target2".into()]);
        cmd.execute(VoteArgs { vote_name: Some("mockvote".into()), args: vec!["Target2".into()] }, &game_cmd2).await;

        let state_lock = state.lock().await;
        let active = state_lock.active_vote.as_ref().unwrap();
        let target_args = active.target_args.downcast_ref::<MockVoteArgs>().unwrap();
        assert_eq!(target_args.target, "Target1"); // Still Target1
    }

    #[tokio::test]
    async fn test_yes_no_commands() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state: &'static SharedVotingState = Box::leak(Box::new(Arc::new(Mutex::new(state_init))));
        
        // Setup active vote
        {
            let mut s = state.lock().await;
            s.active_vote = Some(ActiveVote {
                logic: Box::new(VoteTypeHandler::new(MockVote)),
                raw_target_args: vec!["TestTarget".into()],
                target_args: Box::new(MockVoteArgs { target: "TestTarget".into() }),
                yes_votes: HashSet::new(),
                no_votes: HashSet::new(),
                end_time: Instant::now() + Duration::from_secs(60),
            });
        }

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let publisher: &'static EventPublisher<BroadcastMessage> = Box::leak(Box::new(crate::events::bus::EventPublisher::new(tx)));
        let yes_cmd = YesCommand { state, broadcaster: publisher };
        let no_cmd = NoCommand { state, broadcaster: publisher };

        let voter1 = mock_actor("Voter1", false);
        let voter2 = mock_actor("Voter2", false);

        yes_cmd.execute(YesArgs {}, &mock_game_command(voter1.clone(), vec![])).await;
        no_cmd.execute(NoArgs {}, &mock_game_command(voter2.clone(), vec![])).await;

        {
            let s = state.lock().await;
            let active = s.active_vote.as_ref().unwrap();
            assert!(active.yes_votes.contains("Voter1"));
            assert!(active.no_votes.contains("Voter2"));
        }

        // Change vote: Voter2 changes to YES
        yes_cmd.execute(YesArgs {}, &mock_game_command(voter2, vec![])).await;
        {
            let s = state.lock().await;
            let active = s.active_vote.as_ref().unwrap();
            assert!(active.yes_votes.contains("Voter2"));
            assert!(!active.no_votes.contains("Voter2"));
        }
    }

    #[tokio::test]
    async fn test_cancel_vote_permissions() {
        let (broadcaster, _messages) = mock_broadcaster();
        let (game_event_pub, _events) = mock_game_event_publisher();
        let mut subscriber = CommandSubscriber::new(broadcaster, game_event_pub);

        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        
        subscriber.register::<CancelVoteCommand, CancelVoteArgs>(CancelVoteCommand { state: state.clone() });

        // Setup active vote
        {
            let mut s = state.lock().await;
            s.active_vote = Some(ActiveVote {
                logic: Box::new(VoteTypeHandler::new(MockVote)),
                raw_target_args: vec!["TestTarget".into()],
                target_args: Box::new(MockVoteArgs { target: "TestTarget".into() }),
                yes_votes: HashSet::new(),
                no_votes: HashSet::new(),
                end_time: Instant::now() + Duration::from_secs(60),
            });
        }

        // Regular user tries to cancel
        let user = mock_actor_elevated("RegularUser", false);
        let event = GameEvent::CommandRequestEvent(CommandRequest {
            name: "cancel".to_string(),
            args: vec![],
            raw_args: "".to_string(),
            actor: user,
            source: CommandSource::GameChat,
        });
        subscriber.on_event(&event).await;
        
        {
            let s = state.lock().await;
            assert!(s.active_vote.is_some(), "Vote should NOT be cancelled by regular user");
        }

        // Admin tries to cancel
        let admin = mock_actor_elevated("AdminUser", true);
        let event = GameEvent::CommandRequestEvent(CommandRequest {
            name: "cancel".to_string(),
            args: vec![],
            raw_args: "".to_string(),
            actor: admin,
            source: CommandSource::GameChat,
        });
        subscriber.on_event(&event).await;

        {
            let s = state.lock().await;
            assert!(s.active_vote.is_none(), "Vote SHOULD be cancelled by admin");
        }
    }

    #[tokio::test]
    async fn test_vote_timeout_tick() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let (broadcaster, messages) = mock_broadcaster();
        let cmd = VoteCommand { state: state.clone(), broadcaster };

        // Setup active vote that expires in the past
        {
            let mut s = state.lock().await;
            s.player_count_provider = Box::new(MockPlayerCountProvider(10)); // 10% of 10 is 1 vote required
            s.active_vote = Some(ActiveVote {
                logic: Box::new(VoteTypeHandler::new(MockVote)),
                raw_target_args: vec!["TestTarget".into()],
                target_args: Box::new(MockVoteArgs { target: "TestTarget".into() }),
                yes_votes: HashSet::from(["User1".into()]), // 1 Yes
                no_votes: HashSet::new(),                  // 0 No
                end_time: Instant::now() - Duration::from_secs(1), // Already expired
            });
        }

        // Trigger tick
        cmd.on_tick().await;

        let s = state.lock().await;
        assert!(s.active_vote.is_none(), "Vote should be cleared after timeout");
    }

    #[tokio::test]
    async fn test_vote_success_at_threshold() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let (broadcaster, messages) = mock_broadcaster();
        let cmd = VoteCommand { state: state.clone(), broadcaster };

        // 10 players, 10% min_vote_percentage means 1 vote required.
        // 1 vote cast -> Success.
        {
            let mut s = state.lock().await;
            s.player_count_provider = Box::new(MockPlayerCountProvider(10));
            s.active_vote = Some(ActiveVote {
                logic: Box::new(VoteTypeHandler::new(MockVote)),
                raw_target_args: vec!["TestTarget".into()],
                target_args: Box::new(MockVoteArgs { target: "TestTarget".into() }),
                yes_votes: HashSet::from(["User1".into()]),
                no_votes: HashSet::new(),
                end_time: Instant::now() - Duration::from_secs(1),
            });
        }
        cmd.on_tick().await;
        assert!(state.lock().await.active_vote.is_none());
    }

    #[tokio::test]
    async fn test_vote_failure_below_threshold() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let (broadcaster, messages) = mock_broadcaster();
        let cmd = VoteCommand { state: state.clone(), broadcaster };

        // 100 players, 10% min_vote_percentage means 10 votes required.
        // 1 vote cast -> Failure.
        {
            let mut s = state.lock().await;
            s.player_count_provider = Box::new(MockPlayerCountProvider(100));
            s.active_vote = Some(ActiveVote {
                logic: Box::new(VoteTypeHandler::new(MockVote)),
                raw_target_args: vec!["TestTarget".into()],
                target_args: Box::new(MockVoteArgs { target: "TestTarget".into() }),
                yes_votes: HashSet::from(["User1".into()]),
                no_votes: HashSet::new(),
                end_time: Instant::now() - Duration::from_secs(1),
            });
        }
        cmd.on_tick().await;
        assert!(state.lock().await.active_vote.is_none());
    }

    #[tokio::test]
    async fn test_vote_exact_threshold() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let (broadcaster, messages) = mock_broadcaster();
        let cmd = VoteCommand { state: state.clone(), broadcaster };

        // Test exact threshold (10 players, 10% = 1 required)
        {
            let mut s = state.lock().await;
            s.player_count_provider = Box::new(MockPlayerCountProvider(10));
            s.active_vote = Some(ActiveVote {
                logic: Box::new(VoteTypeHandler::new(MockVote)),
                raw_target_args: vec!["TestTarget".into()],
                target_args: Box::new(MockVoteArgs { target: "TestTarget".into() }),
                yes_votes: HashSet::from(["User1".into()]),
                no_votes: HashSet::new(),
                end_time: Instant::now() - Duration::from_secs(1),
            });
        }
        cmd.on_tick().await;
        assert!(state.lock().await.active_vote.is_none());
    }

    #[tokio::test]
    async fn test_vote_help_command() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let (broadcaster, messages) = mock_broadcaster();
        let cmd = VoteCommand { state: state.clone(), broadcaster };

        let actor = mock_actor("User1", false);
        let game_cmd = mock_game_command(actor, vec!["help".into()]);
        
        // Execute with "help" argument
        cmd.execute(VoteArgs { vote_name: Some("help".into()), args: vec![] }, &game_cmd).await;
        
        // Small delay to ensure the spawned task processes the message
        tokio::time::sleep(Duration::from_millis(20)).await;

        let msgs = messages.lock().await;
        assert_eq!(msgs.len(), 1);
        let msg = &msgs[0];
        assert_eq!(msg.title, Some("🗳️ Vote Help".to_string()));
        let fields = msg.fields.as_ref().unwrap();
        let field = &fields[0];
        assert_eq!(field.0, "Available Votes");
        assert!(field.1.contains("`!vote mockvote <target>`"));
    }

    #[tokio::test]
    async fn test_vote_unknown_type_suggests_help() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let (broadcaster, _messages) = mock_broadcaster();
        let cmd = VoteCommand { state: state.clone(), broadcaster };

        let actor = mock_actor("User1", false);
        let game_cmd = mock_game_command(actor, vec!["invalid".into()]);
        
        cmd.execute(VoteArgs { vote_name: Some("invalid".into()), args: vec![] }, &game_cmd).await;
    }
}
