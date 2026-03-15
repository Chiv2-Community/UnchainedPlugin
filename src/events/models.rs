use serenity::all::{RoleId, UserId};
use strum::{Display, IntoStaticStr};
use crate::game::chivalry2::EChatType;
use crate::swarn;

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


#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
pub enum ChatSource { Discord, Console, Game }

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
pub enum ChatType { Admin, Global, Team }
impl ChatType {
    pub fn from_chiv_chat_type(echat_type: EChatType) -> Self {
        match echat_type {
            EChatType::Admin => ChatType::Admin,
            EChatType::AllSay => ChatType::Global,
            EChatType::TeamSay => ChatType::Team,
            _ => {
                swarn!("Unhandled chat type {}. Falling back to Global chat", echat_type);
                ChatType::Global // Default to global... Clean this over time.
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameChatMessage {
    pub chat_source: ChatSource,
    pub chat_type: ChatType,
    pub sender: String,
    pub message: String,
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
            permissions: ActorPermissions { flags: if is_admin { PermissionFlags::ADMIN} else { PermissionFlags::USER} },
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
    JoinEvent(Join),
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
