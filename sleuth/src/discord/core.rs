use async_trait::async_trait;
use serenity::all::{Http, ChannelId, CreateMessage};
use std::sync::Arc;
use std::any::Any;

use crate::discord::responses::{BotResponse, NO_RESP};

/// The base trait for anything that happens in the game.
pub trait GameEvent: Send + Sync + 'static {
    /// Required for modules to "downcast" the event to its concrete type.
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Unique ID for filtering in the JSON config (e.g., "Join", "Kill").
    fn event_type(&self) -> &'static str;
    
    fn sanitize(&mut self) {}

    /// OPTIONAL: If this event is a simple notification, override this 
    /// to return the message. This is the "Automatic" part.
    fn to_notification(&self) -> Option<CreateMessage> {
        None
    }
}

/// A Subscriber (Module) that listens to the event stream.
#[async_trait]
pub trait DiscordSubscriber: Send + Sync {
    /// Unique name used for enabling/disabling via config.
    fn name(&self) -> &'static str;

    /// Called for every event. Returns an optional message to send to Discord.
    async fn on_event(&mut self, event: &dyn GameEvent, http: &Arc<Http>, channel: ChannelId) -> Vec<BotResponse>;

    /// Optional periodic task.
    async fn on_tick(&mut self, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        NO_RESP
    }
}