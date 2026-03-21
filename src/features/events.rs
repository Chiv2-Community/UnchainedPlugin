use std::sync::Arc;
use std::time::Duration;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use crate::events::models::GameEvent;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::{EventBus, EventPublisher, Subscriber};
use async_trait::async_trait;
use crate::modules::vote::{NoCommand, SharedVotingState, VoteCommand, VotingState, YesCommand};
// use crate::modules::votes::votekick::KickVote;
use crate::modules::votes::votemap::MapVote;
use crate::modules::votes::votebots::{AddBotsVote, NoBotsVote};
use crate::events::broadcast_message::console::ConsoleChatBroadcastSubscriber;
use crate::events::broadcast_message::game_chat::InGameChatBroadcastSubscriber;
use crate::modules::command::CommandSubscriber;
use crate::modules::chat::{ChatRelaySubscriber, GameChatSink, ConsoleChatSink};
use crate::modules::killstreak::KillstreakSubscriber;
use crate::modules::event_broadcast::EventBroadcastSubscriber;
use crate::modules::stats::{StatsTrackerSubscriber, StatsTrackerState, TopCommand, MyStatsCommand};
use crate::modules::duel::DuelManagerSubscriber;
use crate::modules::say::SayCommand;
use crate::modules::cmd::CmdCommand;
// use crate::modules::discord::admin_alert::AdminAlertModule;
// use crate::modules::discord::dashboard::DashboardSubscriber;
use crate::modules::votes::votespeed::SpeedVote;
use crate::modules::votes::votemod::ModVote;

use crate::modules::mod_dump::DumpModsCommand;
use crate::modules::game_info::GameInfoCommand;
use crate::modules::mod_list::ListModsCommand;
use crate::modules::votes::endmap::EndMapVote;

pub struct UnchainedEventSystem {
    // Used to distribute game events to various subscribers for arbitrary processing
    pub game_event_bus: Arc<EventBus<GameEvent>>,
    pub game_event_publisher: EventPublisher<GameEvent>,

    // Used to distribute message broadcasts to various subscribers to send messages to different backends
    pub message_broadcast_event_bus: Arc<EventBus<BroadcastMessage>>,
    pub message_broadcast_event_publisher: EventPublisher<BroadcastMessage>,

    pub chat_relay_subscriber: ChatRelaySubscriber,
    pub command_subscriber: Arc<Mutex<CommandSubscriber>>,
}

pub struct CommandSubscriberProxy {
    inner: Arc<Mutex<CommandSubscriber>>,
}

#[async_trait]
impl Subscriber<GameEvent> for CommandSubscriberProxy {
    fn identifier(&self) -> &'static str {
        "CommandSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        let mut sub = self.inner.lock().await;
        sub.on_event(event).await;
    }

    async fn on_tick(&mut self) {
        let mut sub = self.inner.lock().await;
        sub.on_tick().await;
    }
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

    let command_subscriber = Arc::new(Mutex::new(CommandSubscriber::new(
        Box::leak(Box::new(message_broadcast_event_publisher.clone())),
        Box::leak(Box::new(game_event_publisher.clone()))
    )));

    UnchainedEventSystem {
        game_event_bus,
        game_event_publisher: game_event_publisher.clone(),
        message_broadcast_event_bus,
        message_broadcast_event_publisher: message_broadcast_event_publisher.clone(),
        chat_relay_subscriber,
        command_subscriber,
    }
}

pub async fn initialize_subscribers() {
    let _ = &*EVENT_SYSTEM; // Ensure EVENT_SYSTEM is initialized
    let game_event_bus = &EVENT_SYSTEM.game_event_bus;
    let broadcast_message_bus = &EVENT_SYSTEM.message_broadcast_event_bus;
    let _game_event_publisher = &EVENT_SYSTEM.game_event_publisher;
    let broadcast_message_publisher = &EVENT_SYSTEM.message_broadcast_event_publisher;

    {
        let mut command_subscriber = EVENT_SYSTEM.command_subscriber.lock().await;
        initalize_vote_commands(&mut command_subscriber, broadcast_message_publisher).await;

        // Discord ported commands
        let stats_state = Arc::new(Mutex::new(StatsTrackerState::new("stats.json")));
        command_subscriber.register(TopCommand::new(stats_state.clone(), broadcast_message_publisher));
        command_subscriber.register(MyStatsCommand::new(stats_state.clone(), broadcast_message_publisher));
        command_subscriber.register(CmdCommand);
        command_subscriber.register(SayCommand::new(broadcast_message_publisher));
        command_subscriber.register(DumpModsCommand);
        command_subscriber.register(GameInfoCommand);
        command_subscriber.register(ListModsCommand);
    }

    let _ = broadcast_message_bus.subscribe(Box::new(ConsoleChatBroadcastSubscriber {})).await;
    let _ = broadcast_message_bus.subscribe(Box::new(InGameChatBroadcastSubscriber {})).await;

    let _ = game_event_bus.subscribe(Box::new(CommandSubscriberProxy { inner: EVENT_SYSTEM.command_subscriber.clone() })).await;
    let _ = game_event_bus.subscribe(Box::new(EVENT_SYSTEM.chat_relay_subscriber.clone())).await;

    // Discord ported subscribers
    let stats_state = Arc::new(Mutex::new(StatsTrackerState::new("stats.json"))); // Need it again for subscriber
    let _ = game_event_bus.subscribe(Box::new(KillstreakSubscriber::new(broadcast_message_publisher))).await;
    let _ = game_event_bus.subscribe(Box::new(EventBroadcastSubscriber::new(broadcast_message_publisher))).await;
    let _ = game_event_bus.subscribe(Box::new(StatsTrackerSubscriber::new(stats_state, broadcast_message_publisher))).await;
    let _ = game_event_bus.subscribe(Box::new(DuelManagerSubscriber::new(broadcast_message_publisher))).await;

}

async fn initalize_vote_commands(command_subscriber: &mut CommandSubscriber, broadcast_event_publisher: &'static EventPublisher<BroadcastMessage>) {
    let shared_state: &'static SharedVotingState = Box::leak(Box::new(SharedVotingState::new(Mutex::new(VotingState::new()))));

    command_subscriber.register(YesCommand::new(shared_state, broadcast_event_publisher));
    command_subscriber.register(NoCommand::new(shared_state, broadcast_event_publisher));

    let mut vote_command = VoteCommand::new(
        shared_state.clone(),
        broadcast_event_publisher
    );

    // vote_command.register(KickVote).await; // Delete this? It's built in to the game.
    vote_command.register(EndMapVote).await;
    vote_command.register(MapVote).await;
    vote_command.register(AddBotsVote).await;
    vote_command.register(NoBotsVote).await;
    vote_command.register(SpeedVote).await;
    vote_command.register(ModVote).await;

    command_subscriber.register(vote_command);
}
