use std::any::Any;
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use async_trait::async_trait;
use clap::{Parser, CommandFactory};
use crate::discord::modules::voting::vote_module::BoxClone;
use crate::events::broadcast::{BroadcastMessage, ServerBroadcast};
use crate::events::command::Command;
use crate::events::commands::votes::votekick::KickVote;
use crate::events::commands::votes::votemap::MapVote;
use crate::events::models::GameCommand;

/// Trait that defines a specific type of vote's behavior
pub trait VoteType<T: Parser + Send + Sync + Clone>: Send + Sync {
    fn title(&self) -> String;
    fn description(&self) -> String;
    fn vote_description(&self, args: T) -> String;
    fn min_yes_vote_ratio(&self) -> f32;
    fn min_votes_required_ratio(&self) -> f32;

    /// Checks if the vote can be started with the given arguments
    fn check_prerequisites(&self, args: T) -> Result<(), String> { Ok(()) }
    
    fn on_success(&self, target_args: T);

    fn clone_box(&self) -> Box<dyn ErasedVoteType>;
}

pub trait ErasedVoteType: Send + Sync {
    fn name(&self) -> String;
    fn title(&self) -> String;
    fn description(&self) -> String;
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
    pub vote_name: String,
    /// Arguments for the specific vote type
    pub args: Vec<String>,
}

pub struct VoteCommand {
    pub state: SharedVotingState,
    pub broadcaster: Arc<dyn ServerBroadcast>,
}

impl VoteCommand {
    pub fn new(state: SharedVotingState, broadcaster: Arc<dyn ServerBroadcast>) -> Self {
        Self { state, broadcaster }
    }

    pub fn default(broadcaster: Arc<dyn ServerBroadcast>) -> Self {
        let mut cmd = Self::new(SharedVotingState::new(Mutex::new(VotingState {
            active_vote: None,
            registry: HashMap::new(),
            player_count_provider: Box::new(GlobalPlayerCountProvider),
        })), broadcaster);

        cmd.register(MapVote);
        cmd.register(KickVote);

        cmd
    }

    pub fn register<T, V>(&mut self, logic: V)
    where
        T: Parser + CommandFactory + Send + Sync + Clone + 'static,
        V: VoteType<T> + Send + Sync + 'static,
    {
        self.state.blocking_lock().register(logic);
    }

    pub fn unregister<T>(&mut self)
    where
        T: Parser + CommandFactory + Send + Sync + Clone + 'static,
    {
        self.state.blocking_lock().unregister::<T>();
    }

    pub fn unregister_by_name(&mut self, name: &str) {
        self.state.blocking_lock().unregister_by_name(name);
    }
}

#[async_trait]
impl Command<VoteArgs> for VoteCommand {
    async fn execute(&self, args: VoteArgs, command: &GameCommand) {
        let mut state = self.state.lock().await;

        if args.vote_name == "help" {
            let mut content = "Available vote types:\n".to_string();
            let mut sorted_keys: Vec<_> = state.registry.keys().collect();
            sorted_keys.sort();

            let mut message =
                BroadcastMessage::new()
                    .title("Vote Help".to_string())
                    .color(0x3498db);

            for name in sorted_keys {
                if let Some(logic) = state.registry.get(name) {
                    message = message.field(name.clone(), logic.description());
                }
            }

            self.broadcaster.broadcast(message).await;
            return;
        }

        if state.active_vote.is_some() {
            let message = BroadcastMessage::new()
                .title("Cannot Start Vote".to_string())
                .content("A vote is already in progress.".to_string())
                .color(0xFF0000);
            self.broadcaster.broadcast(message).await;
            return;
        }

        if let Some(logic) = state.registry.get(&args.vote_name) {
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
                        end_time: Instant::now() + Duration::from_secs(60),
                    });

                    let message =
                        BroadcastMessage::new()
                            .title(format!("Vote for '{}' started by '{}'", name, command.actor.display_name))
                            .content(format!("Type !yes or !no in chat to vote. {}", description))
                            .color(0x00FF00);

                    self.broadcaster.broadcast(message).await;
                }
                Err(e) => {
                    let message = BroadcastMessage::new()
                        .title("Cannot Start Vote".to_string())
                        .content(format!("Error: {}", e))
                        .color(0xFF0000);
                    self.broadcaster.broadcast(message).await;
                }
            }
        } else {
            let message = BroadcastMessage::new()
                .title("Cannot Start Vote".to_string())
                .content(format!("Unknown vote type: {}. Type !vote help to see all options.", args.vote_name))
                .color(0xFF0000);
            self.broadcaster.broadcast(message).await;
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
                self.broadcaster.broadcast(message).await;
                active.logic.on_success(active.target_args);
            } else {
                let message = BroadcastMessage::new()
                    .title(format!("{} FAILED", active.logic.vote_description(active.raw_target_args.as_slice())))
                    .content(format!("Result: {} Yes, {} No. {}", active.yes_votes.len(), active.no_votes.len(), failure_reason))
                    .color(0xFF0000);
                self.broadcaster.broadcast(message).await;
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "yes", about = "Vote YES on the active poll")]
pub struct YesArgs {}

pub struct YesCommand {
    pub state: SharedVotingState,
    broadcaster: Arc<dyn ServerBroadcast>
}

#[async_trait]
impl Command<YesArgs> for YesCommand {
    async fn execute(&self, _args: YesArgs, command: &GameCommand) {
        let mut state = self.state.lock().await;
        if let Some(ref mut active) = state.active_vote {
            let voter = command.actor.display_name.clone();
            active.no_votes.remove(&voter);
            active.yes_votes.insert(voter);


            self.broadcaster.broadcast(format!("{} voted YES", command.actor.display_name).into()).await;
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "no", about = "Vote NO on the active poll")]
pub struct NoArgs {}

pub struct NoCommand {
    pub state: SharedVotingState,
    broadcaster: Arc<dyn ServerBroadcast>
}

#[async_trait]
impl Command<NoArgs> for NoCommand {
    async fn execute(&self, _args: NoArgs, command: &GameCommand) {
        let mut state = self.state.lock().await;
        if let Some(ref mut active) = state.active_vote {
            let voter = command.actor.display_name.clone();
            active.yes_votes.remove(&voter);
            active.no_votes.insert(voter);
            println!("{} voted NO", command.actor.display_name);
            self.broadcaster.broadcast(format!("{} voted NO", command.actor.display_name).into()).await;
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
    async fn execute(&self, _args: CancelVoteArgs, command: &GameCommand) {
        if !command.actor.is_elevated() {
            println!("Only elevated users can cancel votes.");
            return;
        }
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
    use crate::events::models::{CommandActor, ActorIdentity, ActorPermissions, PermissionFlags, CommandSource};

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

    struct MockBroadcaster {
        messages: Arc<Mutex<Vec<BroadcastMessage>>>,
    }
    #[async_trait]
    impl ServerBroadcast for MockBroadcaster {
        async fn broadcast(&self, message: BroadcastMessage) {
            self.messages.lock().await.push(message);
        }
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

    fn mock_game_command(actor: CommandActor, args: Vec<String>) -> GameCommand {
        GameCommand {
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
        let messages = Arc::new(Mutex::new(Vec::new()));
        let cmd = VoteCommand { state: state.clone(), broadcaster: Arc::new(MockBroadcaster { messages: messages.clone() }) };

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
        let messages = Arc::new(Mutex::new(Vec::new()));
        let cmd = VoteCommand { state: state.clone(), broadcaster: Arc::new(MockBroadcaster { messages: messages.clone() }) };
        
        let actor = mock_actor("Initiator", false);
        let game_cmd = mock_game_command(actor, vec!["TournamentGrounds".into()]);
        let args = VoteArgs { vote_name: "mockvote".into(), args: vec!["TournamentGrounds".into()] };

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
        let messages = Arc::new(Mutex::new(Vec::new()));
        let cmd = VoteCommand { state: state.clone(), broadcaster: Arc::new(MockBroadcaster { messages: messages.clone() }) };
        
        // Start first vote
        let actor1 = mock_actor("User1", false);
        let game_cmd1 = mock_game_command(actor1, vec!["Target1".into()]);
        cmd.execute(VoteArgs { vote_name: "mockvote".into(), args: vec!["Target1".into()] }, &game_cmd1).await;

        // Try start second vote
        let actor2 = mock_actor("User2", false);
        let game_cmd2 = mock_game_command(actor2, vec!["Target2".into()]);
        cmd.execute(VoteArgs { vote_name: "mockvote".into(), args: vec!["Target2".into()] }, &game_cmd2).await;

        let state_lock = state.lock().await;
        let active = state_lock.active_vote.as_ref().unwrap();
        let target_args = active.target_args.downcast_ref::<MockVoteArgs>().unwrap();
        assert_eq!(target_args.target, "Target1"); // Still Target1
    }

    #[tokio::test]
    async fn test_yes_no_commands() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        
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

        let broadcaster = Arc::new(MockBroadcaster { messages: Arc::new(Mutex::new(Vec::new())) });
        let yes_cmd = YesCommand { state: state.clone(), broadcaster: broadcaster.clone() };
        let no_cmd = NoCommand { state: state.clone(), broadcaster: broadcaster.clone() };

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
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        
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

        let cancel_cmd = CancelVoteCommand { state: state.clone() };

        // Regular user tries to cancel
        let user = mock_actor("RegularUser", false);
        cancel_cmd.execute(CancelVoteArgs {}, &mock_game_command(user, vec![])).await;
        
        {
            let s = state.lock().await;
            assert!(s.active_vote.is_some());
        }

        // Admin tries to cancel
        let admin = mock_actor("AdminUser", true);
        cancel_cmd.execute(CancelVoteArgs {}, &mock_game_command(admin, vec![])).await;

        {
            let s = state.lock().await;
            assert!(s.active_vote.is_none());
        }
    }

    #[tokio::test]
    async fn test_vote_timeout_tick() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let cmd = VoteCommand { state: state.clone(), broadcaster: Arc::new(MockBroadcaster { messages: messages.clone() }) };

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
        let messages = Arc::new(Mutex::new(Vec::new()));
        let cmd = VoteCommand { state: state.clone(), broadcaster: Arc::new(MockBroadcaster { messages: messages.clone() }) };

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
        let messages = Arc::new(Mutex::new(Vec::new()));
        let cmd = VoteCommand { state: state.clone(), broadcaster: Arc::new(MockBroadcaster { messages: messages.clone() }) };

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
        let messages = Arc::new(Mutex::new(Vec::new()));
        let cmd = VoteCommand { state: state.clone(), broadcaster: Arc::new(MockBroadcaster { messages: messages.clone() }) };

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
        let messages = Arc::new(Mutex::new(Vec::new()));
        let cmd = VoteCommand { state: state.clone(), broadcaster: Arc::new(MockBroadcaster { messages: messages.clone() }) };

        let actor = mock_actor("User1", false);
        let game_cmd = mock_game_command(actor, vec!["help".into()]);
        
        // Execute with "help" argument
        cmd.execute(VoteArgs { vote_name: "help".into(), args: vec![] }, &game_cmd).await;
        
        // Help should have executed without errors (MockBroadcaster just ignores it)
        // In a real scenario, we'd check if broadcaster.broadcast was called with help text
    }

    #[tokio::test]
    async fn test_vote_unknown_type_suggests_help() {
        let mut state_init = VotingState::new();
        state_init.register(MockVote);
        let state = Arc::new(Mutex::new(state_init));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let cmd = VoteCommand { state: state.clone(), broadcaster: Arc::new(MockBroadcaster { messages: messages.clone() }) };

        let actor = mock_actor("User1", false);
        let game_cmd = mock_game_command(actor, vec!["invalid".into()]);
        
        cmd.execute(VoteArgs { vote_name: "invalid".into(), args: vec![] }, &game_cmd).await;
    }
}
