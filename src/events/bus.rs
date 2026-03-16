use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;


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

    /// Optional periodic task delay. If None, on_tick will not be called.
    fn tick_delay(&self) -> Option<Duration> {
        None
    }

    /// Called periodically if tick_delay returns Some.
    async fn on_tick(&mut self) {}
}

pub struct EventBus<E>
{
    subscribers: Arc<Mutex<Vec<Box<dyn Subscriber<E>>>>>
}

impl<E> EventBus<E>
    where E: Send + Sync + 'static + Clone,
{
    pub fn new(subscribers: Vec<Box<dyn Subscriber<E>>>) -> Result<Self, String> {
        let mut seen_ids = std::collections::HashSet::new();
        for sub in &subscribers {
            let id = sub.identifier();
            if !seen_ids.insert(id) {
                return Err(format!("Duplicate subscriber identifier: {}", id));
            }
        }

        Ok(Self {
            subscribers: Arc::new(Mutex::new(subscribers))
        })
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

    pub fn start(self: Arc<Self>) -> EventPublisher<E> {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<E>();
        let bus = Arc::clone(&self);
        
        // Event processing task
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                bus.dispatch(event).await;
            }
        });

        // Tick management task
        let bus_for_ticks = Arc::clone(&self);
        tokio::spawn(async move {
            let mut last_ticks: HashMap<String, Instant> = HashMap::new();
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;
                // Use a separate scope to ensure the lock is dropped
                let mut subscribers_to_tick = Vec::new();
                {
                    let mut subscribers = bus_for_ticks.subscribers.lock().await;
                    let now = Instant::now();
                    for (index, subscriber) in subscribers.iter_mut().enumerate() {
                        if let Some(delay) = subscriber.tick_delay() {
                            let id = format!("{}_{}", subscriber.identifier(), index);
                            let last_tick = last_ticks.get(&id).cloned();

                            if last_tick.is_none() || now.duration_since(last_tick.unwrap()) >= delay {
                                subscribers_to_tick.push(index);
                                last_ticks.insert(id, now);
                            }
                        }
                    }
                }

                if !subscribers_to_tick.is_empty() {
                    let mut subscribers = bus_for_ticks.subscribers.lock().await;
                    for index in subscribers_to_tick {
                        if let Some(subscriber) = subscribers.get_mut(index) {
                            subscriber.on_tick().await;
                        }
                    }
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

        fn tick_delay(&self) -> Option<Duration> {
            self.delay
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

        let bus = Arc::new(EventBus::new(vec![Box::new(sub)]).unwrap());
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
        let sub = sub("test_subscriber").with_delay(200);
        let ticks = Arc::clone(&sub.ticks);

        let bus = Arc::new(EventBus::new(vec![Box::new(sub)]).unwrap());
        let _publisher = Arc::clone(&bus).start();

        // Wait for at least 2 ticks (400ms + some buffer)
        // Resolution is 100ms
        tokio::time::sleep(Duration::from_millis(600)).await;

        let count = *ticks.lock().await;
        // Should have ticked 2 or 3 times depending on exact timing
        assert!(count >= 2, "Expected at least 2 ticks, got {}", count);
    }

    #[tokio::test]
    async fn test_multiple_subscribers_different_delays() {
        let sub1 = sub("sub1").with_delay(200);
        let ticks1 = Arc::clone(&sub1.ticks);

        let sub2 = sub("sub2").with_delay(400);
        let ticks2 = Arc::clone(&sub2.ticks);

        let bus = Arc::new(EventBus::new(vec![]).unwrap());
        let _publisher = Arc::clone(&bus).start();

        bus.subscribe(Box::new(sub1)).await.unwrap();
        bus.subscribe(Box::new(sub2)).await.unwrap();

        tokio::time::sleep(Duration::from_millis(800)).await;

        let count1 = *ticks1.lock().await;
        let count2 = *ticks2.lock().await;

        // First tick is immediate when they are first seen in the loop
        // sub1 (200ms): 0ms (tick 1), 200ms (tick 2), 400ms (tick 3), 600ms (tick 4), 800ms (tick 5)
        // sub2 (400ms): 0ms (tick 1), 400ms (tick 2), 800ms (tick 3)
        assert!(count1 >= 3, "Subscriber 1 expected at least 3 ticks, got {}", count1);
        assert!(count2 >= 2, "Subscriber 2 expected at least 2 ticks, got {}", count2);
        assert!(count1 > count2, "Subscriber 1 should have more ticks than Subscriber 2");
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

        // Should fail during construction
        let bus_res = EventBus::new(vec![
            Box::new(Sub1),
            Box::new(Sub2),
        ]);
        match bus_res {
            Err(e) => assert!(e.contains("Duplicate subscriber identifier: sub")),
            _ => panic!("Should have failed"),
        }

        // Should fail during runtime subscription
        let bus = Arc::new(EventBus::new(vec![Box::new(Sub1)]).unwrap());
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

        let bus = Arc::new(EventBus::new(vec![]).unwrap());
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