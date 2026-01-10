use serenity::all::{ChannelId, CreateEmbed, CreateMessage, Http};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Main,
    Admin,
    General,
    Custom(ChannelId),
}

pub enum ResponseContent {
    Message(CreateMessage),
    Embed(CreateEmbed),
}

impl From<CreateMessage> for ResponseContent {
    fn from(m: CreateMessage) -> Self { Self::Message(m) }
}

impl From<CreateEmbed> for ResponseContent {
    fn from(e: CreateEmbed) -> Self { Self::Embed(e) }
}

pub struct BotResponse {
    pub targets: Vec<Target>,
    pub content: ResponseContent,
}

impl From<CreateMessage> for BotResponse {
    fn from(m: CreateMessage) -> Self {
        Self {
            targets: Vec::new(),
            content: m.into(),           // This uses the From above!
        }
    }
}

impl From<CreateEmbed> for BotResponse {
    fn from(e: CreateEmbed) -> Self {
        Self {
            targets: Vec::new(),
            content: e.into(),
        }
    }
}

impl BotResponse {
    fn new(content: impl Into<ResponseContent>) -> Self {
        Self {
            targets: Vec::new(),
            content: content.into(),
        }
    }

    pub fn to(mut self, target: Target) -> Self {
        self.targets.push(target);
        self
    }

    pub fn to_main(self) -> Self { self.to(Target::Main) }
    pub fn to_admin(self) -> Self { self.to(Target::Admin) }
    pub fn to_general(self) -> Self { self.to(Target::General) }
}

// Global helpers to start a chain
pub fn msg(text: impl Into<String>) -> BotResponse {
    BotResponse::new(CreateMessage::new().content(text))
}

pub fn embed(e: CreateEmbed) -> BotResponse {
    BotResponse::new(e)
}

pub trait IntoResponses {
    fn into_responses(self) -> Vec<BotResponse>;
}

impl IntoResponses for BotResponse {
    fn into_responses(self) -> Vec<BotResponse> { vec![self] }
}

impl IntoResponses for Vec<BotResponse> {
    fn into_responses(self) -> Vec<BotResponse> { self }
}

impl IntoResponses for () {
    fn into_responses(self) -> Vec<BotResponse> { vec![] }
}

// Optional: Support for Option<BotResponse> for easy migration
impl IntoResponses for Option<BotResponse> {
    fn into_responses(self) -> Vec<BotResponse> {
        self.map(|r| vec![r]).unwrap_or_default()
    }
}

pub const NO_RESP: Vec<BotResponse> = Vec::new();

/*
async fn on_event(&mut self, event: &dyn GameEvent) -> impl IntoResponses {
    if event.is_emergency() {
        // Return a single chained response
        return msg("🚨 Emergency!").to_main().to_admin();
    }

    // Return multiple distinct responses
    vec![
        msg("Standard update").to_main(),
        embed(CreateEmbed::new().title("Log Item")).to_admin(),
    ]
}
*/