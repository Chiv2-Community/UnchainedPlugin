use serenity::all::{RoleId, UserId};
use crate::features::discord::DiscordConfig;
use crate::game::chivalry2::{EChatType, EFaction};
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
pub enum ChatSource {
    Discord(ChatType),
    Console(ChatType),
    #[strum(to_string = "Game")]
    Native(NativeChat),
}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, IntoStaticStr)]
pub enum ChatType { Admin, Global, Team }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameChatMessage {
    pub chat_source: ChatSource,
    pub sender: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeChat {
    Player { channel: EChatType, faction: Option<EFaction> },
    System { channel: EChatType },
}

impl GameChatMessage {
    pub fn chat_type(&self) -> ChatType {
        match self.chat_source {
            ChatSource::Discord(channel) | ChatSource::Console(channel) => channel,
            ChatSource::Native(NativeChat::Player { channel, .. }) => match channel {
                EChatType::AllSay => ChatType::Global,
                EChatType::Admin => ChatType::Admin,
                _ => ChatType::Team,
            },
            ChatSource::Native(NativeChat::System { .. }) => ChatType::Global,
        }
    }

    pub fn faction(&self) -> &'static str {
        match self.chat_source {
            ChatSource::Discord(_) => "Discord",
            ChatSource::Native(NativeChat::System { .. }) => "System",
            ChatSource::Native(NativeChat::Player { faction, .. }) => faction.map(EFaction::as_str).unwrap_or("Unknown"),
            ChatSource::Console(_) => "Console",
        }
    }

    pub fn chat_label(&self) -> String {
        let label = match self.chat_source {
            ChatSource::Discord(_) => "Discord",
            ChatSource::Console(channel) => channel.into(),
            ChatSource::Native(NativeChat::System { channel: EChatType::AllSay }) if !self.sender.is_empty() => "All",
            ChatSource::Native(NativeChat::System { .. }) => "System",
            ChatSource::Native(NativeChat::Player { channel, faction }) => match channel {
                EChatType::AllSay => "All",
                EChatType::ServerSay => "System",
                EChatType::Spectator => "Spectator",
                EChatType::TeamSay if faction == Some(EFaction::None) => "Spectator",
                EChatType::TeamSay => self.faction(),
                _ => channel.as_str(),
            },
        };
        label.to_ascii_uppercase()
    }

    /// Only non-command native global player records may be relayed externally.
    pub fn is_relayable(&self) -> bool {
        match self.chat_source {
            ChatSource::Native(NativeChat::Player { channel: EChatType::AllSay, .. }) => !self.message.starts_with('!'),
            ChatSource::Native(_) => false,
            ChatSource::Discord(_) | ChatSource::Console(_) => true,
        }
    }
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

    #[test_case::test_case(false, "Knight"; "human")]
    #[test_case::test_case(true, "(bot) Knight"; "bot")]
    fn combat_actor_display_marks_bots(is_bot: bool, expected: &str) {
        assert_eq!(CombatActor::new("Knight".into(), is_bot).to_string(), expected);
    }

    #[test_case::test_case(&[], "<none>"; "no_arguments")]
    #[test_case::test_case(&[""], ""; "empty_argument")]
    #[test_case::test_case(&["hello", "🩸"], "hello 🩸"; "unicode_arguments")]
    #[test_case::test_case(&["hello world", "", "last"], "hello world  last"; "spaces_and_empty_argument")]
    fn args_display_joins_arguments(args: &[&str], expected: &str) {
        let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        assert_eq!(Args(&args).to_string(), expected);
    }

    #[test_case::test_case(Some(7), &[7], PermissionFlags::ADMIN | PermissionFlags::MODERATOR | PermissionFlags::USER; "matching_role")]
    #[test_case::test_case(Some(7), &[8], PermissionFlags::USER; "different_role")]
    #[test_case::test_case(Some(7), &[], PermissionFlags::USER; "no_roles")]
    #[test_case::test_case(None, &[7], PermissionFlags::USER; "admin_role_unconfigured")]
    fn discord_actor_permissions_follow_configured_admin_role(admin_role_id: Option<u64>, roles: &[u64], expected_flags: PermissionFlags) {
        let config = DiscordConfig {
            bot_token: "unused".into(),
            admin_role_id,
            dashboard_channel_id: None,
            general_chat_channel_id: None,
            admin_notification_channel_id: None,
            event_log_channel_id: None,
            mention_on_admin: false,
        };
        let roles = roles.iter().map(|id| RoleId::new(*id)).collect::<Vec<_>>();
        let actor = CommandActor::from_discord(UserId::new(1), "Player".into(), &roles, &config);
        assert_eq!(actor.permissions.flags, expected_flags);
    }
}
