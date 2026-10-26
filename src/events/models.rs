use serenity::all::{RoleId, UserId};
use crate::features::discord::DiscordConfig;
use crate::game::chivalry2::EChatType;
use strum::{Display, IntoStaticStr};
use itertools::Itertools;
use std::fmt;

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
    pub killer: CombatActor,
    pub victim: CombatActor,
    pub killers: Vec<CombatActor>,
    pub kill_reason: String,
    pub random_seed: i32,
    pub dead_character_id: u8,
    pub attach_to_projectile: bool,
    pub source: Damage,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CombatActor {
    pub name: String,
    pub is_bot: bool,
}

impl CombatActor {
    pub fn new(name: String, is_bot: bool) -> Self {
        Self { name, is_bot }
    }
}

impl fmt::Display for CombatActor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_bot {
            f.write_str("(bot) ")?;
        }
        f.write_str(&self.name)
    }
}

#[derive(Debug, Clone)]
pub struct DamageSource {
    pub amount: f32,
    pub source: String, // weapon or throwable name
    pub attack_type: String // slash/overhead/throw/stab/etc;
}

/// Triggered when the server changes maps
#[derive(Debug, Clone)]
pub struct MapChange {
    pub new_map_url: String,
    pub new_map_name: String,
}

/// Triggered when a match finishes (before the map change)
#[derive(Debug, Clone)]
pub struct MatchEnd {
    pub winner_team: String,
    pub final_score: String,
}


#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
pub enum ChatSource { Discord, Console, Game }

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, IntoStaticStr)]
pub enum ChatType { Admin, Global, Team }
impl ChatType {
    pub fn from_chiv_chat_type(echat_type: EChatType) -> Self {
        match echat_type {
            EChatType::Admin => ChatType::Admin,
            EChatType::AllSay => ChatType::Global,
            EChatType::TeamSay => ChatType::Team,
            _ => {
                crate::warn!("Unhandled chat type {}. Falling back to Global chat", echat_type);
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
pub enum CommandSource {
    GameChat,
    Discord,
    ServerConsole
}

#[derive(Debug, Clone)]
pub struct CommandActor {
    pub identity: ActorIdentity,
    pub permissions: ActorPermissions,
    pub display_name: String,
}

pub struct Args<'a>(pub &'a [String]);

impl fmt::Display for Args<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            f.write_str("<none>")
        } else {
            write!(f, "{}", self.0.iter().format(" "))
        }
    }
}

impl CommandActor {
    pub fn is_admin(&self) -> bool { self.permissions.flags.contains(PermissionFlags::ADMIN) }
    pub fn is_moderator(&self) -> bool { self.permissions.flags.contains(PermissionFlags::MODERATOR) }
    pub fn is_elevated(&self) -> bool { self.is_admin() || self.is_moderator() }
    pub fn from_discord(user_id: UserId, username: String, roles: &[RoleId], config: &DiscordConfig) -> Self {
        let is_admin = config.admin_role_id.map(|id| roles.contains(&RoleId::new(id))).unwrap_or(false);

        Self {
            display_name: username.clone(),
            identity: ActorIdentity::DiscordUser {
                user_id,
                display_name: username,
            },
            permissions: ActorPermissions { flags: if is_admin { PermissionFlags::ADMIN | PermissionFlags::MODERATOR | PermissionFlags::USER } else { PermissionFlags::USER} },
        }
    }
}

#[derive(Debug, Clone, Display)]
pub enum ActorIdentity {
    ServerConsole,
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
pub struct CommandRequest {
    pub name: String,
    pub args: Vec<String>,
    pub raw_args: String,
    pub actor: CommandActor,
    pub source: CommandSource,
}

#[derive(Debug, Clone)]
pub struct CommandExecuted {
    pub name: String,
    pub args: Vec<String>,
    pub raw_args: String,
    pub actor: CommandActor,
    pub source: CommandSource,
}

#[derive(Debug, Clone)]
pub struct CommandRejected {
    pub name: String,
    pub args: Vec<String>,
    pub raw_args: String,
    pub actor: CommandActor,
    pub source: CommandSource,
    pub rejection_reason: String,
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
pub struct Damage {
    pub attacker: String,
    pub victim: String,
    pub attacker_actor: CombatActor,
    pub victim_actor: CombatActor,
    pub damage: DamageSource
}

// --- The Unified GameEvent Enum ---

#[derive(Debug, Clone, IntoStaticStr, Display)]
pub enum GameEvent {
    JoinEvent(Join),
    LeaveEvent(Leave),
    CrashEvent(Crash),
    KillEvent(Kill),
    MapChangeEvent(MapChange),
    MatchEndEvent(MatchEnd), // Never dispatched
    GameChatMessageEvent(GameChatMessage),
    CommandRequestEvent(CommandRequest),
    CommandExecutedEvent(CommandExecuted),
    CommandRejectedEvent(CommandRejected),
    ServerStatusEvent(ServerStatus),
    AdminAlertEvent(AdminAlert),
    DuelStartEvent(DuelStart), // Never dispatched
    AttackEvent(Attack), // Never dispatched
    DamageEvent(Damage),
    ServerRestartEvent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_actor_display_includes_name_and_bot_status() {
        for name in ["Knight", "🩸", ""] {
            let human = CombatActor::new(name.into(), false).to_string();
            let bot = CombatActor::new(name.into(), true).to_string();
            assert!(human.contains(name));
            assert!(bot.contains(name));
            assert_ne!(human, bot, "bot status must be distinguishable");
        }
    }

    #[test]
    fn args_display_includes_argument_values() {
        for args in [
            vec![],
            vec![String::new()],
            vec!["hello".into(), "🩸".into()],
            vec!["hello world".into(), String::new(), "last".into()],
        ] {
            let text = Args(&args).to_string();
            for arg in args.iter().filter(|arg| !arg.is_empty()) {
                assert!(text.contains(arg), "missing {arg:?} in {text:?}");
            }
        }
    }

    #[test]
    fn discord_actor_permissions_follow_configured_admin_role() {
        let mut config = DiscordConfig {
            bot_token: "unused".into(),
            admin_role_id: Some(7),
            dashboard_channel_id: None,
            general_chat_channel_id: None,
            admin_notification_channel_id: None,
            event_log_channel_id: None,
            mention_on_admin: false,
        };
        for (admin_role_id, roles, expected_flags) in [
            (
                Some(7),
                vec![RoleId::new(7)],
                PermissionFlags::ADMIN | PermissionFlags::MODERATOR | PermissionFlags::USER,
            ),
            (Some(7), vec![RoleId::new(8)], PermissionFlags::USER),
            (Some(7), vec![], PermissionFlags::USER),
            (None, vec![RoleId::new(7)], PermissionFlags::USER),
        ] {
            config.admin_role_id = admin_role_id;
            let actor =
                CommandActor::from_discord(UserId::new(1), "Player".into(), &roles, &config);
            assert_eq!(actor.permissions.flags, expected_flags);
        }
    }
}
