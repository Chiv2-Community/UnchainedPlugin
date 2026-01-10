use crate::{discord::core::GameEvent, game::chivalry2::EChatType};
use serenity::all::{CreateEmbed, CreateMessage, RoleId, UserId};
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
    pub user_id: UserId,
    pub user_roles: Vec<RoleId>,
}
impl_event!(CommandRequest);

/// Triggered when a player joins the game server
#[derive(Debug)]
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
#[derive(Debug)]
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
#[derive(Debug)]
pub struct KillEvent {
    pub killer: String,
    pub victim: String,
    pub weapon: String,
}
impl_event!(KillEvent);
// Note: KillEvent does NOT override to_notification because we don't 
// want to spam Discord for every single kill. Modules will handle this.

/// Triggered when the server changes maps
#[derive(Debug)]
pub struct MapChangeEvent { 
    pub new_map: String 
}
impl_event!(MapChangeEvent);

/// Triggered when a match finishes (before the map change)
#[derive(Debug)]
pub struct MatchEndEvent {
    pub winner_team: String,
    pub final_score: String,
}
impl_event!(MatchEndEvent);

#[derive(Debug)]
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

// Chat command (parsed from GameChatMessage)
// pub struct GameChatCommandEvent {
//     pub sender: String,
//     pub name: String,
//     pub args: Vec<String>,
//     pub chat_type: EChatType,
// }

// impl GameEvent for GameChatCommandEvent {
//     fn as_any(&self) -> &dyn Any {
//         self
//     }

//     fn as_any_mut(&mut self) -> &mut dyn Any {
//         self
//     }

//     fn event_type(&self) -> &'static str {
//         "ChatCommand"
//     }
// }

// impl GameChatCommandEvent {
//     pub fn from_chat(chat: &GameChatMessage) -> Option<Self> {
//         let msg = chat.message.trim();

//         let without_bang = msg.strip_prefix('!')?;

//         let mut parts = without_bang.split_whitespace();

//         let name = parts.next()?.to_ascii_lowercase();
//         let args = parts.map(|s| s.to_string()).collect();

//         Some(Self {
//             sender: chat.sender.clone(),
//             name,
//             args,
//             chat_type: chat.chat_type,
//         })
//     }
// }

#[derive(Debug, PartialEq)]
pub enum CommandSource {
    GameChat,
    Discord,
}


#[derive(Debug)]
pub struct CommandActor {
    pub identity: ActorIdentity,
    pub permissions: ActorPermissions,
    pub display_name: String,
}

impl CommandActor {
    pub fn is_admin(&self) -> bool { self.permissions.flags.contains(PermissionFlags::ADMIN) }
    pub fn is_moderator(&self) -> bool { self.permissions.flags.contains(PermissionFlags::MODERATOR) }
    pub fn is_elevated(&self) -> bool { self.permissions.flags != PermissionFlags::USER }
    pub fn from_discord(user_id: UserId, username: String, roles: &[RoleId], config: &super::config::DiscordConfig) -> Self {
        let is_admin = roles.contains(&RoleId::new(config.admin_role_id));

        Self {
            display_name: username.clone(),
            identity: ActorIdentity::DiscordUser {
                user_id,
                display_name: username,
            },
            permissions: ActorPermissions { flags: if is_admin {PermissionFlags::ADMIN} else {PermissionFlags::USER} },
        }
    }
}

#[derive(Debug)]
pub enum ActorIdentity {
    GamePlayer {
        player_id: u64,
        display_name: String,
    },
    DiscordUser {
        user_id: UserId,
        display_name: String,
    },
}

#[derive(Debug)]
pub struct ActorPermissions {
    pub flags: PermissionFlags,
}

bitflags::bitflags! {
    #[derive(Debug, PartialEq)]
    pub struct PermissionFlags: u32 {
        const USER          = 0b00000001;
        const ADMIN         = 0b00000010;
        const MODERATOR     = 0b00000100;
        const START_VOTE    = 0b00001000;
        const FORCE_ACTION  = 0b00010000;
    }
}

// This will be helpful for conversion if other input sources are added
// we could just construct this instead of specific Discord/game chat events
#[derive(Debug)]
pub struct BridgeChatEvent {
    pub message: String,
    pub actor: CommandActor,
    pub source: CommandSource,
}
impl_event!(BridgeChatEvent);

#[derive(Debug)]
pub struct GameCommandEvent {
    pub name: String,
    pub args: Vec<String>,
    pub raw_args: String,
    pub actor: CommandActor,
    pub source: CommandSource,
}

impl GameEvent for GameCommandEvent {
    fn as_any(&self) ->  &dyn Any {
        self
    }
    fn as_any_mut(&mut self) ->  &mut dyn Any {
        self
    }
    fn sanitize(&mut self){}
    
    fn event_type(&self) ->  &'static str {
        "GameCommandEvent"
    }

}

fn parse_command(input: &str) -> Option<(String, Vec<String>, String)> {
    let msg = input.trim();
    let without_bang = msg.strip_prefix('!')?.trim();

    let (name, raw_args) = match without_bang.split_once(char::is_whitespace) {
        Some((n, a)) => (n.to_ascii_lowercase(), a.trim()),
        None => (without_bang.to_ascii_lowercase(), ""), // Command with no args
    };

    let args_vec = raw_args.split_whitespace().map(|s| s.to_string()).collect();

    Some((name, args_vec, raw_args.to_string()))
}


impl GameCommandEvent {
    pub fn from_game_chat(chat: &GameChatMessage, perms: PermissionFlags) -> Option<Self> {
        let (name, args, raw_args) = parse_command(&chat.message)?;

        Some(Self {
            name,
            args,
            raw_args,
            source: CommandSource::GameChat,
            actor: CommandActor {
                display_name: chat.sender.clone(),
                identity: ActorIdentity::GamePlayer {
                    player_id: 0, //chat.player_id, // FIXME
                    display_name: chat.sender.clone(),
                },
                permissions: ActorPermissions { flags: perms },
            },
        })
    }
}

// #[poise::command(prefix_command)]
// async fn relay(ctx: Context<'_>, msg: String) -> Result<(), Error> {
//     let event = CommandRequest {
//         command: msg,
//         user: ctx.author().name.clone(),
//         user_roles: ctx.author().roles.clone(),
//     };

//     dispatcher.send(event).await?;
//     Ok(())
// }


impl GameCommandEvent {
    pub fn from_discord(req: &CommandRequest, perms: PermissionFlags) -> Option<Self> {
        let (name, args, raw_args) = parse_command(&req.command)?;

        Some(Self {
            name,
            args,
            raw_args,
            source: CommandSource::Discord,
            actor: CommandActor {
                display_name: req.user.clone(),
                identity: ActorIdentity::DiscordUser {
                    user_id: req.user_id,
                    display_name: req.user.clone(),
                },
                permissions: ActorPermissions { flags: perms },
            },
        })
    }
}




#[derive(Clone, Debug)]
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
#[derive(Debug)]
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

#[derive(Debug)]
pub struct DuelStartEvent { pub challenger: String, pub opponent: String }
#[derive(Debug)]
pub struct AttackEvent { pub attacker: String, pub attack_type: String, pub was_parried: bool }
#[derive(Debug)]
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