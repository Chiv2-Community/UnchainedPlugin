use std::sync::Arc;
use std::time::Duration;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use crate::events::models::GameEvent;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::{EventBus, EventPublisher};
use crate::events::integrations::game_event::vote::{NoCommand, SharedVotingState, VoteCommand, VotingState, YesCommand};
use crate::events::integrations::game_event::votes::votekick::KickVote;
use crate::events::integrations::game_event::votes::votemap::MapVote;
use crate::events::integrations::game_event::votes::votebots::{AddBotsVote, NoBotsVote};
use crate::events::integrations::broadcast_message::console::ConsoleChatBroadcastSubscriber;
use crate::events::integrations::broadcast_message::game_chat::InGameChatBroadcastSubscriber;
use crate::events::integrations::game_event::command::CommandSubscriber;
use crate::events::integrations::game_event::chat::{ChatRelaySubscriber, GameChatSink, ConsoleChatSink};
use crate::events::integrations::game_event::killstreak::KillstreakSubscriber;
use crate::events::integrations::game_event::join_batcher::JoinBatcherSubscriber;
use crate::events::integrations::game_event::stats::{StatsTrackerSubscriber, StatsTrackerState, TopCommand, MyStatsCommand};
use crate::events::integrations::game_event::duel::DuelManagerSubscriber;
use crate::events::integrations::game_event::herald::{AdminHeraldSubscriber, HeraldState, CmdCommand, SayCommand};

pub struct UnchainedEventSystem {
    // Used to distribute game events to various subscribers for arbitrary processing
    pub game_event_bus: Arc<EventBus<GameEvent>>,
    pub game_event_publisher: EventPublisher<GameEvent>,

    // Used to distribute message broadcasts to various subscribers to send messages to different backends
    pub message_broadcast_event_bus: Arc<EventBus<BroadcastMessage>>,
    pub message_broadcast_event_publisher: EventPublisher<BroadcastMessage>,

    pub chat_relay_subscriber: ChatRelaySubscriber,
}

pub static EVENT_SYSTEM: Lazy<UnchainedEventSystem> = Lazy::new(|| init_event_system());

pub fn init_event_system() -> UnchainedEventSystem {
    initialize_event_buses()
}

fn initialize_event_buses() -> UnchainedEventSystem {
    let game_event_bus = Arc::new(EventBus::new(Duration::from_millis(100)));
    let message_broadcast_event_bus = Arc::new(EventBus::new(Duration::from_millis(100)));

    let game_event_publisher = Arc::clone(&game_event_bus).start();
    let message_broadcast_event_publisher = Arc::clone(&message_broadcast_event_bus).start();

    let chat_relay_subscriber = ChatRelaySubscriber::new(vec![
        Box::new(ConsoleChatSink {}),
        Box::new(GameChatSink {})
    ]);

    UnchainedEventSystem {
        game_event_bus,
        game_event_publisher,
        message_broadcast_event_bus,
        message_broadcast_event_publisher,
        chat_relay_subscriber,
    }
}

pub async fn initialize_subscribers() {
    let _ = &*EVENT_SYSTEM; // Ensure EVENT_SYSTEM is initialized
    let game_event_bus = &EVENT_SYSTEM.game_event_bus;
    let broadcast_message_bus = &EVENT_SYSTEM.message_broadcast_event_bus;
    let broadcast_message_publisher = &EVENT_SYSTEM.message_broadcast_event_publisher;

    let mut command_subscriber = CommandSubscriber::new(broadcast_message_publisher);
    initalize_vote_commands(&mut command_subscriber, broadcast_message_publisher).await;

    // Discord ported commands
    let stats_state = Arc::new(Mutex::new(StatsTrackerState::new("stats.json")));
    command_subscriber.register(TopCommand::new(stats_state.clone(), broadcast_message_publisher));
    command_subscriber.register(MyStatsCommand::new(stats_state.clone(), broadcast_message_publisher));
    command_subscriber.register(CmdCommand);
    command_subscriber.register(SayCommand::new(broadcast_message_publisher));

    let _ = broadcast_message_bus.subscribe(Box::new(ConsoleChatBroadcastSubscriber {})).await;
    let _ = broadcast_message_bus.subscribe(Box::new(InGameChatBroadcastSubscriber {})).await;

    let _ = game_event_bus.subscribe(Box::new(command_subscriber)).await;
    let _ = game_event_bus.subscribe(Box::new(EVENT_SYSTEM.chat_relay_subscriber.clone())).await;

    // Discord ported subscribers
    let _ = game_event_bus.subscribe(Box::new(KillstreakSubscriber::new(broadcast_message_publisher))).await;
    let _ = game_event_bus.subscribe(Box::new(JoinBatcherSubscriber::new(broadcast_message_publisher))).await;
    let _ = game_event_bus.subscribe(Box::new(StatsTrackerSubscriber::new(stats_state, broadcast_message_publisher))).await;
    let _ = game_event_bus.subscribe(Box::new(DuelManagerSubscriber::new(broadcast_message_publisher))).await;
    
    let herald_state = Arc::new(Mutex::new(HeraldState { mention_on_crash: true, mention_on_admin: true }));
    let _ = game_event_bus.subscribe(Box::new(AdminHeraldSubscriber::new(herald_state, broadcast_message_publisher))).await;
}

async fn initalize_vote_commands(command_subscriber: &mut CommandSubscriber, broadcast_event_publisher: &'static EventPublisher<BroadcastMessage>) {
    let shared_state: &'static SharedVotingState = Box::leak(Box::new(SharedVotingState::new(Mutex::new(VotingState::new()))));

    command_subscriber.register(YesCommand::new(shared_state, broadcast_event_publisher));
    command_subscriber.register(NoCommand::new(shared_state, broadcast_event_publisher));

    let mut vote_command = VoteCommand::new(
        shared_state.clone(),
        broadcast_event_publisher
    );

    vote_command.register(KickVote).await;
    vote_command.register(MapVote).await;
    vote_command.register(AddBotsVote).await;
    vote_command.register(NoBotsVote).await;

    command_subscriber.register(vote_command);
}