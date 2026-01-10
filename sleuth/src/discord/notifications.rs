use crate::{discord::core::GameEvent, game::chivalry2::EChatType};
use serenity::all::{CreateMessage, CreateEmbed, RoleId};
use std::any::Any;

/// This macro automates the boilerplate for GameEvents.
/// It uses stringify! to turn the struct name into the event_type string.
macro_rules! impl_event {
    ($name:ident) => {
        impl GameEvent for $name {
            fn as_any(&self) -> &dyn Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn Any { self }

            fn sanitize(&mut self) {}

            fn event_type(&self) -> &'static str {
                stringify!($name)
            }
        }
    };
}

// macro_rules! impl_event {
//     ($name:ident) => {
//         impl GameEvent for $name {
//             fn as_any(&self) -> &dyn Any { self }
//             fn event_type(&self) -> &'static str { stringify!($name) }
//         }
//     };
//     // Version that accepts additional trait methods
//     ($name:ident, { $($extra:item)* }) => {
//         impl GameEvent for $name {
//             fn as_any(&self) -> &dyn Any { self }
//             fn event_type(&self) -> &'static str { stringify!($name) }
//             $($extra)*
//         }
//     };
// }

// --- Event Definitions ---

/// Triggered when a player sends a message in Discord
#[derive(Debug)]
pub struct CommandRequest {
    pub command: String,
    pub user: String,
    pub user_roles: Vec<RoleId>,
}
impl_event!(CommandRequest);

/// Triggered when a player joins the game server
pub struct JoinEvent {
    pub name: String,
}
impl_event!(JoinEvent);

// For simple notifications, we only override to_notification
impl JoinEvent {
    fn to_notification(&self) -> Option<CreateMessage> {
        let embed = CreateEmbed::new()
            .title("📥 Reinforcements")
            .description(format!("**{}** has joined the battle!", self.name))
            .color(0x2ecc71);
        Some(CreateMessage::new().add_embed(embed))
    }
}
pub struct CrashEvent {
    pub event_type: String,
    pub event_trace: Vec<String>,
}
impl_event!(CrashEvent);
// impl GameEvent for CrashEvent {
//     fn event_type(&self) -> &'static str { "CrashEvent" }
//     fn as_any(&self) -> &dyn std::any::Any { self }

//     fn to_notification(&self) -> Option<CreateMessage> {
//         crate::sinfo!(f; "to_notification called");
//         let embed = CreateEmbed::new()
//             .title("💀 SERVER CRASH")
//             .description(format!("{}\nTrace:\n```\n{}\n```", self.event_type, self.event_trace.join("\n")))
//             .color(0x2ecc71);
//         Some(CreateMessage::new().add_embed(embed))
//     }
// }

/// Triggered when a kill occurs (Data-heavy event)
pub struct KillEvent {
    pub killer: String,
    pub victim: String,
    pub weapon: String,
}
impl_event!(KillEvent);
// Note: KillEvent does NOT override to_notification because we don't 
// want to spam Discord for every single kill. Modules will handle this.

/// Triggered when the server changes maps
pub struct MapChangeEvent { 
    pub new_map: String 
}
impl_event!(MapChangeEvent);

/// Triggered when a match finishes (before the map change)
pub struct MatchEndEvent {
    pub winner_team: String,
    pub final_score: String,
}
impl_event!(MatchEndEvent);

pub struct GameChatMessage {
    pub sender: String,
    pub message: String,
    pub chat_type: EChatType,
}

impl GameEvent for GameChatMessage {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn sanitize(&mut self) {
        let filter = censor::Censor::Standard;
        // Mutate the fields in place!
        self.message = filter.censor(&self.message);
        self.sender = filter.censor(&self.sender);
    }
    
    fn event_type(&self) ->  &'static str {
        "GameChatMessage"
    }

}

// Optional: If you want these to show up in Discord even if the 
// ChatRelayModule is disabled, implement this:
impl GameChatMessage {
    fn to_notification(&self) -> Option<CreateMessage> {
        // Formats the message for the Discord channel
        Some(CreateMessage::new().content(
            format!("💬 **{}**: {}", self.sender, self.message)
        ))
    }
}

#[derive(Clone)]
pub struct ServerStatus {
    pub name: String,
    pub description: String,
    pub password_protected: bool,
    pub current_map: String,
    pub player_count: i32,
    pub max_players: i32,
    pub mods: Vec<crate::features::Mod>,
    pub active_mods: Vec<crate::features::Mod>
}
impl_event!(ServerStatus);

/// Triggered when a player uses !admin in-game
pub struct AdminAlert {
    pub reporter: String,
    pub reason: String,
}
impl_event!(AdminAlert);

impl AdminAlert {
    fn to_notification(&self) -> Option<CreateMessage> {
        Some(CreateMessage::new().content(format!("🚨 **Admin Request**: {} reports: {}", self.reporter, self.reason)))
    }
}

pub struct DuelStartEvent { pub challenger: String, pub opponent: String }
pub struct AttackEvent { pub attacker: String, pub attack_type: String, pub was_parried: bool }
pub struct DamageEvent { pub attacker: String, pub victim: String, pub damage: f32 }

impl_event!(DuelStartEvent);
impl_event!(AttackEvent);
impl_event!(DamageEvent);
// USAGE
// // In your game's Join Hook
// pub fn on_player_joined(name: &str) {
//     if let Some(bridge) = crate::discord::DISCORD_HANDLE.get() {
//         bridge.dispatch(JoinEvent { name: name.to_string() });
//     }
// }

// // In your game's Kill Hook
// pub fn on_player_kill(killer: &str, victim: &str, weapon: &str) {
//     if let Some(bridge) = crate::discord::DISCORD_HANDLE.get() {
//         bridge.dispatch(KillEvent {
//             killer: killer.to_string(),
//             victim: victim.to_string(),
//             weapon: weapon.to_string(),
//         });
//     }
// }

// pub struct BountyEvent { 
//     pub target: String, 
//     pub reward: String,
//     pub is_claimed: bool,
//     pub slayer: Option<String>
// }

// impl Notification for BountyEvent {
//     fn type_id(&self) -> &'static str { "Bounty" }
//     fn as_any(&self) -> &dyn std::any::Any { self }

//     fn create_message(&self) -> serenity::all::CreateMessage {
//         let mut embed = CreateEmbed::new();
        
//         if !self.is_claimed {
//             embed = embed
//                 .title("💰 BOUNTY PLACED")
//                 .description(format!("A price has been put on **{}**'s head!", self.target))
//                 .color(0xe74c3c); // Red
//         } else {
//             embed = embed
//                 .title("💀 BOUNTY CLAIMED")
//                 .description(format!("**{}** has slain the target **{}**!", self.slayer.as_ref().unwrap(), self.target))
//                 .color(0x2ecc71); // Green
//         }

//         CreateMessage::new().add_embed(embed)
//     }
// }