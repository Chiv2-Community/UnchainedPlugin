//! The event bus is a lightweight asynchronous event distribution system.
//!
//! It is used to send `game_events` to various subscribers that can react to game state changes,
//! commands, or periodic ticks.
//!
//! # Architecture
//!
//! - [`EventBus`]: The central coordinator that manages subscribers and dispatches events.
//! - [`EventPublisher`]: A handle used to send events into the bus. It can be easily cloned and shared.
//! - [`Subscriber`]: A trait implemented by components that want to receive events or receive periodic ticks.
//!
//! # Usage in Project
//!
//! The main event system is initialized in `src/features/events.rs`.
//!
//! There are two primary buses used in the project:
//! 1. **Game Event Bus**: Distributes [`GameEvent`](crate::events::models::GameEvent) to modules like
//!    [`KillstreakSubscriber`], [`StatsTrackerSubscriber`], and [`CommandSubscriber`].
//! 2. **Broadcast Event Bus**: Distributes [`BroadcastMessage`](crate::events::broadcast::BroadcastMessage)
//!    to backends (Discord, Game Chat, Console) via subscribers like [`ConsoleChatBroadcastSubscriber`].
//!
//! Events are dispatched asynchronously, and subscribers can also receive periodic `on_tick` calls
//! at a rate configured when the bus is created.
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;
use anyhow::Result;


#[derive(Clone)]
pub struct EventPublisher<T> {
    bus: UnboundedSender<T>
}
impl<T> EventPublisher<T> {
    pub fn new(bus: UnboundedSender<T>) -> Self {
        Self { bus }
    }

    pub fn publish(&self, event: T) {
        let _ = self.bus.send(event);
    }
}

#[async_trait]
pub trait Subscriber<T>: Send + Sync {
    /// Unique name/identifier string used for enabling/disabling via config.
    fn identifier(&self) -> &'static str;

    /// Called for every event. Returns an optional message to send to Discord.
    async fn on_event(&mut self, event: &T);

    /// Called periodically at the bus's tick rate.
    async fn on_tick(&mut self) {}
}

#[async_trait]
impl Subscriber<crate::events::models::GameEvent> for Arc<Mutex<crate::modules::command::CommandSubscriber>> {
    fn identifier(&self) -> &'static str {
        "CommandSubscriber"
    }

    async fn on_event(&mut self, event: &crate::events::models::GameEvent) {
        let mut sub = self.lock().await;
        sub.on_event(event).await;
    }

    async fn on_tick(&mut self) {
        let mut sub = self.lock().await;
        sub.on_tick().await;
    }
}

pub struct EventBus<E>
{
    subscribers: Arc<Mutex<Vec<Box<dyn Subscriber<E>>>>>,
    tick_rate: Duration,
}

impl<E> EventBus<E>
    where E: Send + Sync + 'static + Clone,
{
    pub fn new(tick_rate: Duration) -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(vec![])),
            tick_rate,
        }
    }

    pub async fn subscribe(&self, subscriber: Box<dyn Subscriber<E>>) -> Result<(), String> {
        let mut subscribers = self.subscribers.lock().await;
        let id = subscriber.identifier();
        if subscribers.iter().any(|s| s.identifier() == id) {
            return Err(format!("Duplicate subscriber identifier: {}", id));
        }
        subscribers.push(subscriber);
        Ok(())
    }

    pub async fn unsubscribe_by_id(&self, name: &str) {
        let mut subscribers = self.subscribers.lock().await;
        subscribers.retain(|s| s.identifier() != name);
    }

    pub fn get_subscribers(&self) -> Arc<Mutex<Vec<Box<dyn Subscriber<E>>>>> {
        Arc::clone(&self.subscribers)
    }

    pub fn start(self: Arc<Self>) -> EventPublisher<E> {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<E>();
        let bus = Arc::clone(&self);

        let handle = tokio::runtime::Handle::try_current()
            .unwrap_or_else(|_| crate::features::tokio_runtime::TOKIO_RUNTIME_HANDLE.clone());
        
        // Event processing task
        handle.spawn(async move {
            while let Some(event) = receiver.recv().await {
                bus.dispatch(event).await;
            }
        });

        // Tick management task
        let bus_for_ticks = Arc::clone(&self);
        let tick_rate = self.tick_rate;
        handle.spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;
                let mut subscribers = bus_for_ticks.subscribers.lock().await;
                for subscriber in subscribers.iter_mut() {
                    subscriber.on_tick().await;
                }
            }
        });

        EventPublisher::new(sender)
    }


    async fn dispatch(&self, event: E) {
        let mut subscribers = self.subscribers.lock().await;
        for subscriber in subscribers.iter_mut() {
            subscriber.on_event(&event).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    #[derive(Clone, Debug, PartialEq)]
    enum TestEvent {
        Hello,
        World,
    }

    struct TestSubscriber {
        name: &'static str,
        events: Arc<Mutex<Vec<TestEvent>>>,
        ticks: Arc<Mutex<u32>>,
        delay: Option<Duration>,
    }

    #[async_trait]
    impl Subscriber<TestEvent> for TestSubscriber {
        fn identifier(&self) -> &'static str {
            self.name
        }

        async fn on_event(&mut self, event: &TestEvent) {
            let mut events = self.events.lock().await;
            events.push(event.clone());
        }

        async fn on_tick(&mut self) {
            let mut ticks = self.ticks.lock().await;
            *ticks += 1;
        }
    }

    fn sub(name: &'static str) -> TestSubscriber {
        TestSubscriber {
            name,
            events: Arc::new(Mutex::new(Vec::new())),
            ticks: Arc::new(Mutex::new(0)),
            delay: None,
        }
    }

    impl TestSubscriber {
        fn with_delay(mut self, millis: u64) -> Self {
            self.delay = Some(Duration::from_millis(millis));
            self
        }
    }

    #[tokio::test]
    async fn test_event_dispatch() {
        let sub = sub("test_subscriber");
        let events = Arc::clone(&sub.events);

        let bus = Arc::new(EventBus::new(Duration::from_millis(100)));
        bus.subscribe(Box::new(sub)).await.expect("Failed to subscribe");
        let publisher = Arc::clone(&bus).start();

        publisher.publish(TestEvent::Hello);
        publisher.publish(TestEvent::World);

        // Give some time for dispatch
        tokio::time::sleep(Duration::from_millis(50)).await;

        let received = events.lock().await;
        assert_eq!(received.len(), 2);
        assert_eq!(received[0], TestEvent::Hello);
        assert_eq!(received[1], TestEvent::World);
    }

    #[tokio::test]
    async fn test_periodic_tick() {
        let sub = sub("test_subscriber");
        let ticks = Arc::clone(&sub.ticks);

        let bus = Arc::new(EventBus::<TestEvent>::new(Duration::from_millis(100)));
        bus.subscribe(Box::new(sub)).await.unwrap();
        let _publisher = Arc::clone(&bus).start();

        // Wait for at least 2 ticks (200ms + some buffer)
        tokio::time::sleep(Duration::from_millis(350)).await;

        let count = *ticks.lock().await;
        // Should have ticked about 3 times
        assert!(count >= 2, "Expected at least 2 ticks, got {}", count);
    }

    #[tokio::test]
    async fn test_multiple_subscribers_same_rate() {
        let sub1 = sub("sub1");
        let ticks1 = Arc::clone(&sub1.ticks);

        let sub2 = sub("sub2");
        let ticks2 = Arc::clone(&sub2.ticks);

        let bus = Arc::new(EventBus::<TestEvent>::new(Duration::from_millis(100)));
        let _publisher = Arc::clone(&bus).start();

        bus.subscribe(Box::new(sub1)).await.unwrap();
        bus.subscribe(Box::new(sub2)).await.unwrap();

        tokio::time::sleep(Duration::from_millis(350)).await;

        let count1 = *ticks1.lock().await;
        let count2 = *ticks2.lock().await;

        assert!(count1 >= 2, "Subscriber 1 expected at least 2 ticks, got {}", count1);
        assert!(count2 >= 2, "Subscriber 2 expected at least 2 ticks, got {}", count2);
    }

    #[tokio::test]
    async fn test_identifier_uniqueness() {
        struct Sub1;
        #[async_trait]
        impl Subscriber<TestEvent> for Sub1 {
            fn identifier(&self) -> &'static str { "sub" }
            async fn on_event(&mut self, _: &TestEvent) {}
        }

        struct Sub2;
        #[async_trait]
        impl Subscriber<TestEvent> for Sub2 {
            fn identifier(&self) -> &'static str { "sub" } // SAME ID
            async fn on_event(&mut self, _: &TestEvent) {}
        }

        let bus = Arc::new(EventBus::<TestEvent>::new(Duration::from_millis(100)));

        // Should fail during runtime subscription
        bus.subscribe(Box::new(Sub1)).await.unwrap();
        let sub2_res = bus.subscribe(Box::new(Sub2)).await;
        match sub2_res {
            Err(e) => assert!(e.contains("Duplicate subscriber identifier: sub")),
            _ => panic!("Should have failed"),
        }
    }

    #[tokio::test]
    async fn test_subscribe_unsubscribe() {
        let sub = sub("test_subscriber");
        let events = Arc::clone(&sub.events);

        let bus = Arc::new(EventBus::<TestEvent>::new(Duration::from_millis(100)));
        let publisher = Arc::clone(&bus).start();
        let id = sub.identifier();

        bus.subscribe(Box::new(sub)).await.unwrap();
        publisher.publish(TestEvent::Hello);
        tokio::time::sleep(Duration::from_millis(50)).await;

        {
            let received = events.lock().await;
            assert_eq!(received.len(), 1);
        }

        bus.unsubscribe_by_id(id).await;
        publisher.publish(TestEvent::World);
        tokio::time::sleep(Duration::from_millis(50)).await;

        {
            let received = events.lock().await;
            assert_eq!(received.len(), 1, "Should not have received more events after unsubscribe");
        }
    }
}
