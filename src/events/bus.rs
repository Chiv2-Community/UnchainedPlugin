use std::collections::HashMap;
use std::sync::Arc;
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
}

pub struct EventBus<E>
{
    subscribers: Arc<Mutex<Vec<Box<dyn Subscriber<E>>>>>
}

impl<E> EventBus<E>
    where E: Send + Sync + 'static + Clone,
{
    pub fn new(subscribers: Vec<Box<dyn Subscriber<E>>>) -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(subscribers))
        }
    }

    pub fn subscribe(&self, subscriber: Box<dyn Subscriber<E>>) {
        let mut subscribers = self.subscribers.blocking_lock();
        subscribers.push(subscriber);
    }

    pub fn unsubscribe_by_name(&self, name: &str) {
        let mut subscribers = self.subscribers.blocking_lock();
        subscribers.retain(|s| s.identifier() != name);
    }

    pub fn start(self: Arc<Self>) -> EventPublisher<E> {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<E>();
        let bus = Arc::clone(&self);
        
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                bus.dispatch(event).await;
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