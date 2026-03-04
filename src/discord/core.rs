use async_trait::async_trait;
use serenity::all::{Http, ChannelId};
use std::sync::Arc;

use crate::discord::responses::{BotResponse, NO_RESP};

pub use crate::discord::events::GameEvent;

/// A Subscriber (Module) that listens to the event stream.
#[async_trait]
pub trait DiscordSubscriber: Send + Sync {
    /// Unique name used for enabling/disabling via config.
    fn name(&self) -> &'static str;

    fn reconfigure(&mut self, _config: &super::config::DiscordConfig) {}

    fn get_commands(&self) -> Vec<super::responses::CommandInfo> {
        vec![] 
    }

    /// Called for every event. Returns an optional message to send to Discord.
    async fn on_event(&mut self, event: &crate::discord::events::GameEvent, http: &Arc<Http>, channel: ChannelId) -> Vec<BotResponse>;

    /// Optional periodic task.
    async fn on_tick(&mut self, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        NO_RESP
    }
}

#[macro_export]
macro_rules! impl_reconfigure {
    ($settings_type:ty) => {
        fn name(&self) -> &'static str { <$settings_type>::key() }

        fn reconfigure(&mut self, config: &DiscordConfig) {
            let key = <$settings_type>::key();
            match config.get_module_config::<$settings_type>() {
                Some(new_settings) => {
                    self.settings = new_settings;
                    println!("[{}] Configuration updated.", key);
                }
                None => {
                    eprintln!("[{}] No configuration found in file, keeping current settings.", key);
                }
            }
        }
    };
}