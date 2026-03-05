use crate::game::chivalry2::EChatType;
use serenity::all::{CreateEmbed, CreateMessage, RoleId, UserId};
use strum::IntoStaticStr;

// --- Event Data Structs ---

// TODO: Move this outside of the discord module. Event dispatch and handling is not necessarily specific to discord. Many different potential modules could benefit from this

/// Triggered when a player sends a message in Discord
#[derive(Debug, Clone)]
pub struct CommandRequest {
    pub command: String,
    pub user: String,
    pub user_id: UserId,
    pub user_roles: Vec<RoleId>,
}

/// Triggered when a player joins the game server
#[derive(Debug, Clone)]
pub struct Join {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Leave {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Crash {
    pub event_type: String,
    pub event_trace: Vec<String>,
}

/// Triggered when a kill occurs (Data-heavy event)
#[derive(Debug, Clone)]
pub struct Kill {
    pub killer: String,
    pub victim: String,
    pub weapon: String,
}

/// Triggered when the server changes maps
#[derive(Debug, Clone)]
pub struct MapChange {
    pub new_map: String 
}

/// Triggered when a match finishes (before the map change)
#[derive(Debug, Clone)]
pub struct MatchEnd {
    pub winner_team: String,
    pub final_score: String,
}

#[derive(Debug, Clone)]
pub struct GameChatMessage {
    pub sender: String,
    pub message: String,
    pub chat_type: EChatType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandSource {
    GameChat,
    Discord,
}

#[derive(Debug, Clone)]
pub struct CommandActor {
    pub identity: ActorIdentity,
    pub permissions: ActorPermissions,
    pub display_name: String,
}

impl CommandActor {
    pub fn is_admin(&self) -> bool { self.permissions.flags.contains(PermissionFlags::ADMIN) }
    pub fn is_moderator(&self) -> bool { self.permissions.flags.contains(PermissionFlags::MODERATOR) }
    pub fn is_elevated(&self) -> bool { self.is_admin() || self.is_moderator() }
    pub fn from_discord(user_id: UserId, username: String, roles: &[RoleId], config: &crate::discord::config::DiscordConfig) -> Self {
        let is_admin = config.admin_role_id.map(|id| roles.contains(&RoleId::new(id))).unwrap_or(false);

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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct ActorPermissions {
    pub flags: PermissionFlags,
}

bitflags::bitflags! {
    #[derive(Debug, PartialEq, Clone)]
    pub struct PermissionFlags: u32 {
        const USER          = 0b00000001;
        const ADMIN         = 0b00000010;
        const MODERATOR     = 0b00000100;
        const START_VOTE    = 0b00001000;
        const FORCE_ACTION  = 0b00010000;
    }
}

#[derive(Debug, Clone)]
pub struct BridgeChat {
    pub message: String,
    pub actor: CommandActor,
    pub source: CommandSource,
}

#[derive(Debug, Clone)]
pub struct GameCommand {
    pub name: String,
    pub args: Vec<String>,
    pub raw_args: String,
    pub actor: CommandActor,
    pub source: CommandSource,
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

/// Triggered when a player uses !admin in-game
#[derive(Debug, Clone)]
pub struct AdminAlert {
    pub reporter: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct DuelStart { pub challenger: String, pub opponent: String }
#[derive(Debug, Clone)]
pub struct Attack { pub attacker: String, pub attack_type: String, pub was_parried: bool }
#[derive(Debug, Clone)]
pub struct Damage { pub attacker: String, pub victim: String, pub damage: f32 }

#[derive(Debug, Clone)]
pub struct MapVote {
    pub initiator: String,
    pub map_target: String,
}

#[derive(Debug, Clone)]
pub struct VoteCast {
    pub voter_id: String,
    pub choice: bool, // true = Yes, false = No
}

// --- The Unified GameEvent Enum ---

#[derive(Debug, Clone, IntoStaticStr)]
pub enum GameEvent {
    CommandRequestEvent(CommandRequest),
    JoinEvent(Join), // Never dispatched
    LeaveEvent(Leave), // Never dispatched
    CrashEvent(Crash),
    KillEvent(Kill), // Never dispatched
    MapChangeEvent(MapChange),
    MatchEndEvent(MatchEnd), // Never dispatched
    GameChatMessageEvent(GameChatMessage),
    BridgeChatEvent(BridgeChat), // Never dispatched
    GameCommandEvent(GameCommand),
    ServerStatusEvent(ServerStatus),
    AdminAlertEvent(AdminAlert),
    DuelStartEvent(DuelStart), // Never dispatched
    AttackEvent(Attack), // Never dispatched
    DamageEvent(Damage), // Never dispatched
    MapVoteEvent(MapVote), // Never dispatched
    VoteCastEvent(VoteCast), // Never dispatched
}

impl GameEvent {
    pub fn sanitized(mut self) -> Self {
        match self {
            GameEvent::GameChatMessageEvent(ref mut chat) => {
                let filter = censor::Censor::Standard;

                GameEvent::GameChatMessageEvent(GameChatMessage {
                    sender: filter.censor(&chat.sender),
                    message: filter.censor(&chat.message),
                    chat_type: chat.chat_type
                })
            }
            other => other
        }
    }

    pub fn dispatch(self, handle: Option<&crate::discord::DiscordHandle>) {
        if let Some(handle) = handle {
            handle.dispatch(self);
        } else if let Some(handle) = crate::discord::DISCORD_HANDLE.get() {
            handle.dispatch(self);
        }
    }

    pub fn to_notification(&self) -> Option<CreateMessage> {
        match self {
            GameEvent::JoinEvent(e) => {
                let embed = CreateEmbed::new()
                    .title("📥 Reinforcements")
                    .description(format!("**{}** has joined the battle!", e.name))
                    .color(0x2ecc71);
                Some(CreateMessage::new().add_embed(embed))
            },
            GameEvent::AdminAlertEvent(e) => {
                Some(CreateMessage::new().content(format!("🚨 **Admin Request**: {} reports: {}", e.reporter, e.reason)))
            },
            GameEvent::MapVoteEvent(e) => {
                Some(CreateMessage::new().content(
                    format!("{} started a vote to change map to {}", e.initiator, e.map_target)
                ))
            }
            _ => None,
        }
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


impl GameCommand {
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
                    player_id: 0, //chat.player_id, // FIXME: grab playerid from the game
                    display_name: chat.sender.clone(),
                },
                permissions: ActorPermissions { flags: perms },
            },
        })
    }

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
// USAGE
// // In your game's Join Hook
// pub fn on_player_joined(name: &str) {
//     if let Some(bridge) = crate::discord::DISCORD_HANDLE.get() {
//         bridge.dispatch(Join { name: name.to_string() });
//     }
// }

// // In your game's Kill Hook
// pub fn on_player_kill(killer: &str, victim: &str, weapon: &str) {
//     if let Some(bridge) = crate::discord::DISCORD_HANDLE.get() {
//         bridge.dispatch(Kill {
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