#![allow(non_snake_case)]
use std::{os::raw::c_void, str::FromStr};
use bitflags::bitflags;
use strum::Display;
use crate::{game::engine::FText, ue::{FString, FVector, TArray, UObject}};

#[repr(C)]
#[derive(Debug)]
pub struct AActor {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Debug)]
pub struct ATBLPlayerController { 
    _private: [u8; 0x1348],
	pub bOnlineInventoryInitialized: bool,
	pub bPlayerCustomizationReceived: bool,
}

#[repr(C)]
#[derive(Debug)]
pub struct AController {
    // 0x0000 -> 0x0260: inherited AActor data (layout simplified as padding)
    pub _base_actor_padding: [u8; 0x260],
    // 0x0260: APlayerState* PlayerState (UE4 Controller.h)
    pub player_state: *mut APlayerState,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum EKillReason {
    Damage = 0,
    FallDamage = 1,
    Suicide = 2,
    TapOut = 3,
    OutOfCombat = 4,
    FellOutOfWorld = 5,
    Disconnect = 6,
    ForwardSpawn = 7,
    SwitchedTeams = 8,
    Spectator = 9,
}

impl EKillReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            EKillReason::Damage => "Damage",
            EKillReason::FallDamage => "FallDamage",
            EKillReason::Suicide => "Suicide",
            EKillReason::TapOut => "TapOut",
            EKillReason::OutOfCombat => "OutOfCombat",
            EKillReason::FellOutOfWorld => "FellOutOfWorld",
            EKillReason::Disconnect => "Disconnect",
            EKillReason::ForwardSpawn => "ForwardSpawn",
            EKillReason::SwitchedTeams => "SwitchedTeams",
            EKillReason::Spectator => "Spectator",
        }
    }
}

impl TryFrom<u8> for EKillReason {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(EKillReason::Damage),
            1 => Ok(EKillReason::FallDamage),
            2 => Ok(EKillReason::Suicide),
            3 => Ok(EKillReason::TapOut),
            4 => Ok(EKillReason::OutOfCombat),
            5 => Ok(EKillReason::FellOutOfWorld),
            6 => Ok(EKillReason::Disconnect),
            7 => Ok(EKillReason::ForwardSpawn),
            8 => Ok(EKillReason::SwitchedTeams),
            9 => Ok(EKillReason::Spectator),
            _ => Err(()),
        }
    }
}

/// Layout based on `CXXHeaderDump/TBL.hpp` (`Size: 0x148`).
#[repr(C)]
#[derive(Debug)]
pub struct FDamageTakenEvent {
    pub damage: f32,
    pub new_stat_value: f32,
    pub damage_source: *mut UObject,  // UDamageSource*
    pub damage_causer: *mut AActor,   // AActor*
    pub damage_taker: *mut AActor,    // AActor*
    pub damage_instigator: *mut AActor, // AActor*
    pub b_killing_blow: bool,
    pub b_suicide: bool,
    pub b_back_stab: bool,
    pub b_entered_kill_volume: bool,
    pub b_lose_limb_cheat: bool,
    pub b_switched_teams_in_loadout_volume: bool,
    pub _pad_after_flags: [u8; 0x2], // 0x002E
    pub hit_result: [u8; 0x88], // 0x0030
    pub ability_spec: *mut c_void, // 0x00B8
    pub abilities_table_row: crate::ue::FName, // 0x00C0
    pub hit_direction: crate::ue::FVector, // 0x00C8 (FVector_NetQuantizeNormal)
    pub damage_taker_combat_state: crate::ue::FName, // 0x00D4
    pub apply_condition: u8, // EConditionType, 0x00DC
    pub _pad_after_apply_condition: [u8; 0x3], // 0x00DD
    pub post_damage_info: [u8; 0x0C], // 0x00E0
    pub _pad_before_inventory_item: [u8; 0x4], // 0x00EC
    pub inventory_item: *mut c_void, // 0x00F0
    pub projectile: *mut c_void, // 0x00F8
    pub location_based_damage: u8, // ELocationBasedDamage, 0x0100
    pub _pad_after_location_based_damage: [u8; 0x7], // 0x0101
    pub attach_parent: *mut c_void, // 0x0108
    pub b_parried: bool, // 0x0110
    pub _pad_after_b_parried: [u8; 0x7], // 0x0111
    pub character_who_parried: *mut c_void, // 0x0118
    pub b_is_in_team_thwack_range: bool, // 0x0120
    pub _pad_before_gore_event: [u8; 0x3], // 0x0121
    pub gore_event: [u8; 0x1C], // 0x0124
    pub kill_reason: EKillReason, // 0x0140
    pub b_arrow_parried: bool, // 0x0141
    pub b_disarmed: bool, // 0x0142
    pub _pad_end: [u8; 0x5], // 0x0143
}

/// Layout based on `CXXHeaderDump/TBL.hpp` (`Size: 0x160`).
#[repr(C)]
#[derive(Debug)]
pub struct FDeathDamageTakenEvent {
    pub damage_taken: FDamageTakenEvent,
    pub killers: TArray<*mut c_void>, // 0x0148
    pub random_seed: i32, // 0x0158
    pub kill_reason: EKillReason, // 0x015C
    pub dead_character_id: u8, // 0x015D
    pub b_attach_to_projectile: bool, // 0x015E
    pub _pad_end: [u8; 0x1], // 0x015F
}

const _: [(); 0x148] = [(); std::mem::size_of::<FDamageTakenEvent>()];
const _: [(); 0x160] = [(); std::mem::size_of::<FDeathDamageTakenEvent>()];

// Chat type enum
// FIXME: More compact, wtf is this
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[allow(clippy::upper_case_acronyms)]
pub enum EChatType {
    AllSay,
    TeamSay,
    Whisper,
    Admin,
    Objective,
    System,
    ServerSay,
    Debug,
    CrosshairMsg,
    Backend,
    Party,
    Spectator,
    ClosedCaption,
    ClosedCaptionMason,
    ClosedCaptionAgatha,
    MAX,
}

impl FromStr for EChatType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AllSay" => Ok(EChatType::AllSay),
            "TeamSay" => Ok(EChatType::TeamSay),
            "Whisper" => Ok(EChatType::Whisper),
            "Admin" => Ok(EChatType::Admin),
            "Objective" => Ok(EChatType::Objective),
            "System" => Ok(EChatType::System),
            "ServerSay" => Ok(EChatType::ServerSay),
            "Debug" => Ok(EChatType::Debug),
            "CrosshairMsg" => Ok(EChatType::CrosshairMsg),
            "Backend" => Ok(EChatType::Backend),
            "Party" => Ok(EChatType::Party),
            "Spectator" => Ok(EChatType::Spectator),
            "ClosedCaption" => Ok(EChatType::ClosedCaption),
            "ClosedCaptionMason" => Ok(EChatType::ClosedCaptionMason),
            "ClosedCaptionAgatha" => Ok(EChatType::ClosedCaptionAgatha),
            "MAX" => Ok(EChatType::MAX),
            _ => Err(()),
        }
    }
}

impl TryFrom<u8> for EChatType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(EChatType::AllSay),
            1 => Ok(EChatType::TeamSay),
            2 => Ok(EChatType::Whisper),
            3 => Ok(EChatType::Admin),
            4 => Ok(EChatType::Objective),
            5 => Ok(EChatType::System),
            6 => Ok(EChatType::ServerSay),
            7 => Ok(EChatType::Debug),
            8 => Ok(EChatType::CrosshairMsg),
            9 => Ok(EChatType::Backend),
            10 => Ok(EChatType::Party),
            11 => Ok(EChatType::Spectator),
            12 => Ok(EChatType::ClosedCaption),
            13 => Ok(EChatType::ClosedCaptionMason),
            14 => Ok(EChatType::ClosedCaptionAgatha),
            15 => Ok(EChatType::MAX),
            _ => Err(()),
        }
    }
}

impl EChatType {
    // A manual list of all variants for the loop
    pub const ALL: [EChatType; 15] = [
        Self::AllSay, Self::TeamSay, Self::Whisper, Self::Admin,
        Self::Objective, Self::System, Self::ServerSay, Self::Debug,
        Self::CrosshairMsg, Self::Backend, Self::Party, Self::Spectator,
        Self::ClosedCaption, Self::ClosedCaptionMason, Self::ClosedCaptionAgatha
    ];

    // Manual string conversion
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AllSay => "AllSay",
            Self::TeamSay => "TeamSay",
            Self::Whisper => "Whisper",
            Self::Admin => "Admin",
            Self::Objective => "Objective",
            Self::System => "System",
            Self::ServerSay => "ServerSay",
            Self::Debug => "Debug",
            Self::CrosshairMsg => "CrosshairMsg",
            Self::Backend => "Backend",
            Self::Party => "Party",
            Self::Spectator => "Spectator",
            Self::ClosedCaption => "ClosedCaption",
            Self::ClosedCaptionMason => "ClosedCaptionMason",
            Self::ClosedCaptionAgatha => "ClosedCaptionAgatha",
            Self::MAX => "MAX",
        }
    }
}

// Helper functions
pub fn send_ingame_message(message: String, chat_type: Option<EChatType>) {
    use crate::resolvers::messages::o_BroadcastLocalizedChat;
    use crate::resolvers::admin_control::o_FText_AsCultureInvariant;
    use crate::resolvers::etc_hooks::o_GetTBLGameMode;
    
    let chat_type_actual = chat_type.unwrap_or(EChatType::AllSay);
    if let Some(world) = crate::globals().world() {
        let mut settings_fstring = FString::from(message.as_str());
        let mut txt = FText::default();

        let res = TRY_CALL_ORIGINAL!(FText_AsCultureInvariant(&mut txt, &mut settings_fstring));

        let game_mode = TRY_CALL_ORIGINAL!(GetTBLGameMode(world));

        if !game_mode.is_null() {
            TRY_CALL_ORIGINAL!(BroadcastLocalizedChat(game_mode, res, chat_type_actual));
        }
    } else {
        crate::swarn!(f; "send_ingame_message: globals.world was None; skipping chat broadcast");
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ATBLGameMode {
    // 0x0000 -> 0x0340: Inherited from AGameMode
    pub _base_padding: [u8; 0x340],

    // 0x0340 (size: 0x8)
    pub game_mode_settings_class: *mut c_void, // TSubclassOf<UTBLGameModeSettings>
    
    // 0x0348 (size: 0x10)
    pub teams: TArray<*mut c_void>,
    
    // 0x0358 (size: 0x8)
    pub ffa_team: *mut c_void,
    
    // 0x0360 (size: 0x1)
    pub attacking_faction: u8,
    // 0x0361 (size: 0x1)
    pub defending_faction: u8,
    // 0x0362 (size: 0x6)
    pub padding_0: [u8; 0x6],
    
    // 0x0368 (size: 0x8)
    pub default_horse_class: *mut c_void,
    
    // 0x0370 (size: 0x28)
    pub blueprint_debug_menu_component_class_padding: [u8; 0x28],
    
    // 0x0398 (size: 0xC)
    pub padding_1: [u8; 0xC],
    
    // 0x03A4 (size: 0x4)
    pub seamless_travel_end_time: f32,
    
    // 0x03A8 (size: 0x1)
    pub gamemode_type: u8,
    // 0x03A9 (size: 0x1)
    pub create_game_mode_type: u8,
    // 0x03AA (size: 0x1)
    pub b_first_bot_match: bool,
    
    // 0x03AB (size: 0x5)
    pub padding_2: [u8; 0x5],
    
    // 0x03B0 (size: 0x8)
    pub game_scoring_class: *mut c_void,
    // 0x03B8 (size: 0x8)
    pub game_scoring_info: *mut c_void,
    // 0x03C0 (size: 0x8)
    pub always_on_music_manager: *mut c_void,
    // 0x03C8 (size: 0x8)
    pub context_vo_manager: *mut c_void,
    // 0x03D0 (size: 0x8)
    pub rank_based_team_algorithm_manager: *mut c_void,
    
    // 0x03D8 (size: 0x10)
    pub spawn_queuer_classes: TArray<*mut c_void>,
    // 0x03E8 (size: 0x10)
    pub spawn_queuers: TArray<*mut c_void>,
    
    // 0x03F8 (size: 0x10) FTBLGameModeOnPlayerKilled
    pub on_player_killed_padding: [u8; 0x10],
    // 0x0408 (size: 0x10) FTBLGameModeOnHorseKilled
    pub on_horse_killed_padding: [u8; 0x10],
    // 0x0418 (size: 0x10) FTBLGameModeOnPawnRevived
    pub on_pawn_revived_padding: [u8; 0x10],
    // 0x0428 (size: 0x10) FTBLGameModeOnPawnDowned
    pub on_pawn_downed_padding: [u8; 0x10],
    // 0x0438 (size: 0x10) FTBLGameModeOnPlayerPossessed
    pub on_player_possessed_padding: [u8; 0x10],
    // 0x0448 (size: 0x10) FTBLGameModeCleanupAbilityActors
    pub cleanup_ability_actors_padding: [u8; 0x10],
    
    // 0x0458 (size: 0x4)
    pub padding_3: [u8; 0x4],
    
    // 0x045C (size: 0x1 each)
    pub b_performed_deferred_spawn_this_frame: bool,
    pub b_disable_player_name_plates: bool,
    pub b_only_show_names_on_teammates: bool,
    pub b_is_ffa_game_mode: bool,
    
    // 0x0460 (size: 0x4)
    pub post_game_slomo: f32,
    // 0x0464 (size: 0x1)
    pub b_hide_hud_important_messages: bool,
    
    // 0x0465 (size: 0x3)
    pub padding_4: [u8; 0x3],
    
    // 0x0468 (size: 0x4)
    pub time_between_loadout_volume_uses: f32,
    
    // 0x046C (size: 0x1 each)
    pub b_use_priority_spawn_settings: bool,
    pub b_use_proximity_spawn_settings: bool,
    pub ignore_class_limits: bool,
    
    // 0x046F (size: 0x1)
    pub padding_5: [u8; 0x1],
    
    // 0x0470 (size: 0x8)
    pub seconds_between_waves_player_count_bonus: *mut c_void,
    // 0x0478 (size: 0x8)
    pub ai_controller_class: *mut c_void,
    // 0x0480 (size: 0x8)
    pub ai_behavior_tree: *mut c_void,
    
    // 0x0488 (size: 0x10) FRespawnStinger
    pub first_spawn_sound_padding: [u8; 0x10],
    // 0x0498 (size: 0x10) FRespawnStinger
    pub respawn_sound_padding: [u8; 0x10],
    
    // 0x04A8 (size: 0x10)
    pub match_id: [u8; 0x10], //FGuid,
    
    // 0x04B8 (size: 0x1)
    pub b_analytics_shutdown: u8,
    // 0x04B9 (size: 0x7)
    pub padding_6: [u8; 0x7],
    
    // 0x04C0 (size: 0x10)
    pub session_banned_players_padding: [u8; 0x10],
    // 0x04D0 (size: 0x10)
    pub community_server_banned_players_padding: [u8; 0x10],
    
    // 0x04E0 (size: 0x1)
    pub bot_backfill_enabled: bool,
    // 0x04E1 (size: 0x3)
    pub padding_7: [u8; 0x3],
    
    // 0x04E4 (size: 0x4 each)
    pub bot_backfill_low_players: i32,
    pub bot_backfill_low_bots: i32,
    pub bot_backfill_high_players: i32,
    pub bot_backfill_high_bots: i32,
    
    // 0x04F4 (size: 0x4 each)
    pub idle_kick_timer_spectate: f32,
    pub idle_kick_timer_disconnect: f32,
    
    // 0x04FC (size: 0x1)
    pub b_horse_compatible_server: bool,
    // 0x04FD (size: 0x3)
    pub padding_8: [u8; 0x3],
    
    // 0x0500 (size: 0x10)
    pub community_server_admin_ids: TArray<FString>,
    
    // 0x0510 (size: 0x1 each)
    pub b_use_open_loadout: bool,
    pub b_respawn_immediately: bool,
    pub b_logging_ability_events: bool,
    
    // 0x0513 (size: 0xD)
    pub padding_9: [u8; 0xD],
    
    // 0x0520 (size: 0x8)
    pub override_ai_objective: *mut c_void,
    
    // 0x0528 (size: 0x138) FMatchComplete
    pub match_complete_event_padding: [u8; 0x138],
    
    // 0x0660 (size: 0x198) FServerPerformanceHistory
    pub server_performance_history_padding: [u8; 0x198],
    
    // 0x07F8 (size: 0x10)
    pub game_mode_modifiers: TArray<*mut c_void>,
    // 0x0808 (size: 0x10)
    pub debug_draw_all_tracers: TArray<*mut c_void>,
    
    // 0x0818 (size: 0x10) FTBLGameModeOnControllerLogout
    pub on_controller_logout_padding: [u8; 0x10],
    // 0x0828 (size: 0x10) FTBLGameModeOnControllerLogin
    pub on_controller_login_padding: [u8; 0x10],
    
    // 0x0838 (size: 0x4)
    pub auto_balance_grace_period_seconds: f32,
    // 0x083C (size: 0x8)
    pub padding_10: [u8; 0x8],
    
    // 0x0844 (size: 0x1 each)
    pub b_enable_auto_demo_recording: bool,
    pub b_is_ai_test_map: bool,
    
    // 0x0846 (size: 0x2)
    pub padding_11: [u8; 0x2],
    
    // 0x0848 (size: 0x10)
    pub maplist: TArray<FString>,
    
    // 0x0858 (size: 0x1)
    pub b_use_maplist: bool,
    // 0x0859 (size: 0x3)
    pub padding_12: [u8; 0x3],
    
    // 0x085C (size: 0x4)
    pub post_match_time: i32,
    
    // 0x0860 (size: 0x10)
    pub server_name: FString,
    // 0x0870 (size: 0x10)
    pub server_identifier: FString,
    
    // 0x0880 (size: 0x4)
    pub map_list_index: i32,
    // 0x0884 (size: 0x4)
    pub padding_13: [u8; 0x4],
    
    // 0x0888 (size: 0x10)
    pub class_limits_padding: [u8; 0x10],
    
    // 0x0898 (size: 0x1 each)
    pub auto_balance_players_by_team_numbers: bool,
    pub auto_balance_players_by_kills: bool,
    
    // 0x089A (size: 0x2)
    pub padding_14: [u8; 0x2],
    
    // 0x089C (size: 0x4)
    pub time_between_team_kill_balance_checks: i32,
    // 0x08A0 (size: 0x4)
    pub time_between_player_num_balance_checks: i32,
    // 0x08A4 (size: 0x4)
    pub start_of_match_grace_period_for_auto_balance: i32,
    // 0x08A8 (size: 0x4)
    pub start_of_match_grace_period_for_team_switching: i32,
    // 0x08AC (size: 0x4)
    pub minimum_kills_modifier_for_auto_balance: f32,
    // 0x08B0 (size: 0x4)
    pub auto_balance_kill_relevancy_time: f32,
    // 0x08B4 (size: 0x4)
    pub auto_balance_kill_threshold: f32,
    // 0x08B8 (size: 0x4)
    pub auto_balance_time_until_forced_respawn: i32,
    
    // 0x08BC (size: 0x1 each)
    pub b_spectators_cannot_send_to_all_chat: bool,
    pub b_client_side_weapon_tracers: bool,
    
    // 0x08BE (size: 0x2)
    pub padding_15: [u8; 0x2],
    
    // 0x08C0 (size: 0x4 each)
    pub time_entered_waiting_to_start: f32,
    pub min_players: i32,
    pub desired_players_to_start_percentage: f32,
    pub min_time_before_starting_match: f32,
    pub max_time_before_starting_match: f32,
    
    // 0x08D4 (size: 0x1 each)
    pub b_manually_start_match: bool,
    pub b_use_prepare_match_timer: bool,
    
    // 0x08D6 (size: 0x2)
    pub padding_16: [u8; 0x2],
    
    // 0x08D8 (size: 0x4)
    pub prepare_match_duration: f32,
    
    // 0x08DC (size: 0x4)
    pub padding_17: [u8; 0x4],
    
    // 0x08E0 (size: 0x8) FTimerHandle
    pub prepare_match_timer_handle: u64,
    
    // 0x08E8 (size: 0x28) TSoftClassPtr<UUserWidget>
    pub game_mode_widget_class_padding: [u8; 0x28],
    
    // 0x0910 (size: 0x4)
    pub epilogue_duration: f32,
    // 0x0914 (size: 0x1)
    pub b_use_strict_team_balance_enforcement: bool,
    
    // 0x0915 (size: 0x3)
    pub padding_18: [u8; 0x3],
    
    // 0x0918 (size: 0x10)
    pub team_balance_options_padding: [u8; 0x10],
    // 0x0928 (size: 0x10)
    pub auto_balance_options_padding: [u8; 0x10],
    // 0x0938 (size: 0x10)
    pub auto_balance_player_priority_config_padding: [u8; 0x10],
    
    // 0x0948 (size: 0x1)
    pub b_use_rank_based_team_assignment: bool,
    // 0x0949 (size: 0x7)
    pub padding_19: [u8; 0x7],
    
    // 0x0950 (size: 0xA8) FTeamImbalanceTracker
    pub team_imbalance_tracker_padding: [u8; 0xA8],
    
    // 0x09F8 (size: 0x1 each)
    pub block_vote_kicking: bool,
    pub is_community_server: bool,
    
    // 0x09FA (size: 0x2)
    pub padding_20: [u8; 0x2],
    
    // 0x09FC (size: 0x8) FName
    pub camera_mode_override_padding: [u8; 0x8],
    
    // 0x0A04 (size: 0x1)
    pub victor: u8,
    
    // 0x0A05 (size: 0x3)
    pub padding_21: [u8; 0x3],
    
    // 0x0A08 (size: 0x10)
    pub pending_player_spawn_queue_padding: [u8; 0x10],
    
    // 0x0A18 (size: 0x10)
    pub padding_22: [u8; 0x10],
    
    // 0x0A28 (size: 0x4)
    pub last_player_spawn_time: f32,
    
    // 0x0A2C (size: 0x4)
    pub padding_23: [u8; 0x4],
    
    // 0x0A30 (size: 0x10)
    pub occupied_player_starts_padding: [u8; 0x10],
    
    // 0x0A40 (size: 0x48)
    pub padding_24: [u8; 0x48],
    
    // 0x0A88 (size: 0x4)
    pub config_warmup_time: i32,
    
    // 0x0A8C (size: 0xC)
    pub padding_25: [u8; 0xC],
    
    // 0x0A98 (size: 0x8)
    pub override_ai_behavior_tree: *mut c_void,
    
    // 0x0AA0 (size: 0x28)
    pub padding_26: [u8; 0x28],
    
    // 0x0AC8 (size: 0x10)
    pub gold_award_by_team_placement_padding: [u8; 0x10],
    
    // 0x0AD8 (size: 0x4)
    pub gold_award_time_period: i32,
    
    // 0x0ADC (size: 0x4)
    pub padding_27: [u8; 0x4],
    
    // 0x0AE0 (size: 0x10)
    pub gold_multiplier_by_daily_hour_padding: [u8; 0x10],
    
    // 0x0AF0 (size: 0x8)
    pub gold_multiplier_by_daily_hour_time_dilation: f64,
    
    // 0x0AF8 (size: 0x4 each)
    pub gold_award_by_time_period_amount: i32,
    pub gold_max_from_playtime_per_game: i32,
    pub xp_max_from_playtime_per_game: i32,
    pub xp_award_time_period: i32,
    
    // 0x0B08 (size: 0x10)
    pub xp_multiplier_by_daily_hour_padding: [u8; 0x10],
    
    // 0x0B18 (size: 0x8)
    pub xp_multiplier_by_daily_hour_time_dilation: f64,
    
    // 0x0B20 (size: 0x4 each)
    pub xp_award_by_time_period_amount: i32,
    pub game_mode_xp_multiplier: f32,
    
    // 0x0B28 (size: 0x228)
    pub padding_28: [u8; 0x228],
    
    // 0x0D50 (size: 0x4)
    pub post_match_matchmaking_mode: i32,
    
    // 0x0D54 (size: 0x4)
    pub padding_29: [u8; 0x4],
    
    // 0x0D58 (size: 0x10)
    pub post_match_travel_string: FString,
    
    // 0x0D68 (size: 0x1)
    pub b_ready_to_post_match_travel: bool,
    
    // 0x0D69 (size: 0xF)
    pub padding_30: [u8; 0xF],
    
    // 0x0D78 (size: 0x8)
    pub game_server_query: *mut c_void,
    
    // 0x0D80 (size: 0x440)
    pub padding_31: [u8; 0x440],
    
    // 0x11C0 (size: 0x50) TMap
    pub friends_by_faction_padding: [u8; 0x50],
    
    // 0x1210 (size: 0x8)
    pub padding_32: [u8; 0x8],
    
    // 0x1218 (size: 0x1)
    pub auto_balance_with_friends_blocked: bool,
}

impl ATBLGameState {
    pub fn get_human_player_count(&self) -> usize {
        let mut count = 0;
        for player_ptr in self.player_array.as_slice() {
            if let Some(player) = unsafe { player_ptr.as_ref() } {
                if !player.base.player_flags.contains(PlayerFlags::IS_A_BOT) {
                    count += 1;
                }
            }
        }
        count
    }
}

pub fn get_human_player_count() -> usize {
    let world_ptr = match crate::globals().world() {
        Some(ptr) => ptr as *mut crate::game::engine::UWorld,
        None => {
            crate::swarn!(f; "get_human_player_count: globals.world was None; returning 0 and skipping player count scan");
            return 0;
        }
    };

    let world = unsafe {
        match world_ptr.as_ref() {
            Some(w) => w,
            None => return 0,
        }
    };

    let game_state = unsafe {
        match world.game_state.as_ref() {
            Some(gs) => gs,
            None => return 0,
        }
    };

    game_state.get_human_player_count()
}

#[repr(C)]
pub struct ATBLGameState {
    // --- 0x0000 -> 0x0258: AActor + AInfo ---
    pub _base_actor_info: [u8; 0x258],

    // --- 0x0258 -> 0x02A8: AGameStateBase ---
    pub game_mode_class: *mut c_void,             // 0x0258
    pub authority_game_mode: *mut c_void,         // 0x0260
    pub spectator_class: *mut c_void,             // 0x0268
    pub player_array: TArray<*mut ATBLPlayerState>,  // 0x0270
    pub b_replicated_has_begun_play: bool,        // 0x0280
    pub padding_base_0: [u8; 0x3],                // 0x0281
    pub replicated_world_time_seconds: f32,       // 0x0284
    pub server_world_time_seconds_delta: f32,     // 0x0288
    pub server_world_time_seconds_update_freq: f32, // 0x028C
    pub padding_base_1: [u8; 0x18],               // 0x0290 -> 0x02A8

    // --- 0x02A8 -> 0x02D8: AGameState ---
    pub match_state: [u8; 0x8],                   // 0x02A8 (FName)
    pub previous_match_state: [u8; 0x8],          // 0x02B0 (FName)
    pub elapsed_time: i32,                        // 0x02B8
    pub padding_gamestate_0: [u8; 0x1C],          // 0x02BC -> 0x02D8

    // --- 0x02D8 -> 0x0998: ATBLGameState ---
    pub teams: TArray<*mut c_void>,         // 0x02D8
    pub ffa_team: *mut c_void,                    // 0x02E8
    pub neutral_team_class: *mut c_void,          // 0x02F0
    pub map_name_padding: [u8; 0x18],             // 0x02F8 (FText)
    pub gamemode_type: u8,                        // 0x0310
    pub padding_0: [u8; 0x7],                     // 0x0311
    pub game_mode_name_padding: [u8; 0x18],       // 0x0318 (FText)
    pub server_name_padding: [u8; 0x18],          // 0x0330 (FText)
    pub platform: FString,                  // 0x0348
    pub victor: u8,                               // 0x0358
    pub padding_1: [u8; 0x7],                     // 0x0359
    pub player_victor: *mut c_void,               // 0x0360
    pub final_match_duration: f32,                // 0x0368
    pub padding_2: [u8; 0x4],                     // 0x036C
    pub next_map_name: FString,             // 0x0370
    
    // Delegates / Events (size: 0x10 each)
    pub on_match_started_padding: [u8; 0x10],     // 0x0380
    pub on_match_ended_padding: [u8; 0x10],       // 0x0390
    pub on_match_won_by_padding: [u8; 0x10],      // 0x03A0
    
    pub padding_3: [u8; 0x18],                    // 0x03B0
    
    pub on_playing_state_begun_padding: [u8; 0x10],   // 0x03C8
    pub on_player_killed_padding: [u8; 0x10],         // 0x03D8
    pub on_player_state_added_padding: [u8; 0x10],    // 0x03E8
    pub on_player_state_removed_padding: [u8; 0x10],  // 0x03F8
    pub on_player_state_uid_replicated_padding: [u8; 0x10], // 0x0408
    pub on_player_state_kills_updated_padding: [u8; 0x10],  // 0x0418
    pub on_character_spawned_padding: [u8; 0x10],     // 0x0428
    pub on_epic_end_game_event_padding: [u8; 0x10],   // 0x0438
    
    pub epic_end_game_state: u8,                  // 0x0448
    pub padding_4: [u8; 0x3],                     // 0x0449
    pub stage_end_time: f32,                      // 0x044C
    pub stage_start_time: f32,                    // 0x0450
    pub padding_5: [u8; 0x4],                     // 0x0454
    pub stage_progress_list_padding: [u8; 0x10],  // 0x0458 (TArray)
    pub server_time_difference: f32,              // 0x0468
    pub b_disable_player_name_plates: bool,       // 0x046C
    pub b_only_show_names_on_teammates: bool,     // 0x046D
    pub b_is_game_mode_ffa: bool,                 // 0x046E
    pub b_disable_team_select: bool,              // 0x046F
    pub b_hide_hud_important_messages: bool,      // 0x0470
    pub padding_6: [u8; 0x3],                     // 0x0471
    pub min_respawn_time: f32,                    // 0x0474
    pub players_needed_to_start_early: i32,       // 0x0478
    pub min_players_to_start: i32,                // 0x047C
    
    pub gameplay_event_msg_class_padding: [u8; 0x28], // 0x0480 (TSoftClassPtr)
    
    // Packed bitfield/flags at 0x04A8
    pub performance_flags: u8,                    // 0x04A8 (bits for weapon tracers, network, etc)
    pub b_server_bad_frame_time: bool,            // 0x04A9
    pub b_use_open_loadout: bool,                 // 0x04AA
    pub padding_7: [u8; 0x1],                     // 0x04AB
    pub progress_bar_primary_team_index: i32,     // 0x04AC
    
    pub class_limits_padding: [u8; 0x10],         // 0x04B0 (TArray)
    pub game_scoring_data_table: *mut c_void,     // 0x04C0
    pub team_score_format_data_table: *mut c_void, // 0x04C8
    pub novelty_score_data_table: *mut c_void,    // 0x04D0
    
    pub first_spawn_sound_padding: [u8; 0x10],    // 0x04D8 (FRespawnStinger)
    pub respawn_sound_padding: [u8; 0x10],        // 0x04E8 (FRespawnStinger)
    pub objective_point_array_padding: [u8; 0x10], // 0x04F8 (TArray)
    pub most_recent_team_score_event_padding: [u8; 0x40], // 0x0508 (FTeamScoreEvent)
    
    pub b_using_new_spawn_system: bool,           // 0x0548
    pub cinematic_state: u8,                      // 0x0549
    pub padding_8: [u8; 0x6],                     // 0x054A
    pub on_cinematic_state_changed_padding: [u8; 0x10], // 0x0550
    pub defender_cinematic_sequence: *mut c_void, // 0x0560
    pub attacker_cinematic_sequence: *mut c_void, // 0x0568
    pub replicated_sequence_bindings_padding: [u8; 0x10], // 0x0570
    
    pub padding_9: [u8; 0xA0],                    // 0x0580
    pub server_cinematic_start_time: f32,         // 0x0620
    pub padding_10: [u8; 0x4],                    // 0x0624
    pub on_match_state_changed_padding: [u8; 0x10], // 0x0628
    pub stage_persistent_msgs_padding: [u8; 0x10], // 0x0638 (TArray)
    pub stage_ending_music_padding: [u8; 0x10],   // 0x0648 (FAKAudioStartStopStruct)
    pub context_objective_message_padding: [u8; 0x68], // 0x0658 (FObjectiveContextMessage)
    pub on_new_objective_msg_padding: [u8; 0x10], // 0x06C0
    pub voting_manager_component: *mut c_void,    // 0x06D0
    pub b_disable_spawning_bots: bool,            // 0x06D8
    pub padding_11: [u8; 0x7],                    // 0x06D9
    pub lobby_id: FString,                  // 0x06E0
    pub on_post_match_end_time_changed_padding: [u8; 0x10], // 0x06F0
    pub ps5_match_id: FString,              // 0x0700
    pub on_ps5_match_id_updated_padding: [u8; 0x10], // 0x0710
    pub ps5_match_responsible_player_padding: [u8; 0x28], // 0x0720 (FUniqueNetIdRepl)
    
    pub padding_12: [u8; 0x10],                   // 0x0748
    pub warmup_end_time: f32,                     // 0x0758
    pub preparing_match_end_time: f32,            // 0x075C
    pub post_match_end_time: f32,                 // 0x0760
    pub post_match_matchmaking_start_server: f32, // 0x0764
    pub b_block_vote_kicking: u8,                 // 0x0768
    pub b_is_community_server: u8,                // 0x0769
    pub padding_13: [u8; 0x2],                    // 0x076A
    pub camera_mode_override: [u8; 0x8],          // 0x076C (FName)
    
    pub padding_14: [u8; 0xA8],                   // 0x0774
    pub b_was_notified_waiting: bool,             // 0x081C
    pub b_delay_garbage_collection: bool,         // 0x081D
    pub padding_15: [u8; 0x22],                   // 0x081E
    
    pub zone_authorities_padding: [u8; 0x10],     // 0x0840 (TArray)
    pub torn_off_items_padding: [u8; 0x50],       // 0x0850 (TMap)
    pub server_torn_off_items_padding: [u8; 0x50], // 0x08A0 (TMap)
    pub padding_16: [u8; 0x50],                   // 0x08F0
    pub current_players_in_parties_padding: [u8; 0x50], // 0x0940 (TMap)
    pub xp_reward_modifier: f32,                  // 0x0990
    pub gold_reward_modifier: f32,                // 0x0994
}

// #[repr(C)]
// pub struct FUniqueNetId {
//     pub padding_14: [u8; 0x30],
//     pub epicid: FString,
//     pub productid: FString,
//     pub rawbytes: [u8; 0x20]
// }

// #[repr(C)]
// pub struct FUniqueNetIdWrapper {
//     pub base: FUniqueNetId
// }
// FIXME: What's the actual structure here?
#[repr(C)]
pub struct FUniqueNetIdRepl {
    // pub replication_bytes: TArray<u8>, // 0x0018
    // pub base: FUniqueNetId,
    // pub replication_bytes: [u8; 0x10]
    pub vtable: *const usize,           // 0x0000
    pub internal_wrapper: [u8; 0x10],   // 0x0008 (FUniqueNetIdWrapper base)
    pub replication_bytes: TArray<u8>, // 0x0018
} // Size: 0x28

bitflags! {
    // Note: Use 'Default' or manual names for pretty printing
    #[derive(Debug, Clone, Copy)]
    pub struct PlayerFlags: u8 {
        const SHOULD_UPDATE_PING = 1 << 0;
        const IS_SPECTATOR       = 1 << 1;
        const ONLY_SPECTATOR     = 1 << 2;
        const IS_A_BOT           = 1 << 3;
        const IS_INACTIVE        = 1 << 4;
        const FROM_PREVIOUS      = 1 << 5;
    }
}

#[repr(C)]
pub struct APlayerState {
    // 0x0000 -> 0x0258: Inherited from AInfo/AActor/UObject
    pub base_padding: [u8; 0x258], 
    
    pub score: f32,                     // 0x0258
    pub player_id: i32,                 // 0x025C
    pub ping: u8,                       // 0x0260
    pub _padding_0: u8,                 // 0x0261
    
    // Bitfields/Flags
    // pub player_flags: u8,               // 0x0262 (Spectator, Bot, Inactive, etc.)
    pub player_flags: PlayerFlags,
    pub _padding_1: u8,                 // 0x0263
    
    pub start_time: i32,                // 0x0264
    pub engine_message_class: *mut c_void, // 0x0268 (ULocalMessage*)
    pub _padding_2: [u8; 0x8],          // 0x0270
    pub saved_network_address: FString, // 0x0278
    pub unique_id: FUniqueNetIdRepl,    // 0x0288 (Size: 0x28)
    pub _padding_3: [u8; 0x8],          // 0x02B0
    pub pawn_private: *mut c_void,      // 0x02B8 (APawn*)
    pub _padding_4: [u8; 0x78],         // 0x02C0
    pub player_name_private: FString,   // 0x0338
    pub _padding_last: [u8; 0x10],
} // Size: 0x348 (Wait, your dump says 0x358, likely 0x10 bytes padding at end)

// impl APlayerState {
//     pub fn should_update_replicated_ping(&self) -> bool {
//         (self.player_flags & (1 << 0)) != 0
//     }

//     pub fn is_spectator(&self) -> bool {
//         (self.player_flags & (1 << 1)) != 0
//     }

//     pub fn only_spectator(&self) -> bool {
//         (self.player_flags & (1 << 2)) != 0
//     }

//     pub fn is_a_bot(&self) -> bool {
//         (self.player_flags & (1 << 3)) != 0
//     }

//     pub fn is_inactive(&self) -> bool {
//         (self.player_flags & (1 << 4)) != 0
//     }

//     pub fn from_previous_level(&self) -> bool {
//         (self.player_flags & (1 << 5)) != 0
//     }
// }

#[repr(C)]
pub struct ATBLPlayerState {
    pub base: APlayerState,             // 0x0000 (Size: 0x358 approx)
    pub _align_to_stats: [u8; 0x8],     // 0x0358 -> 0x0360 (alignment)

    pub online_stats: [u8; 0xD8],       // 0x0360 (FTBLOnlineStats)
    pub online_stats_updated: [u8; 0x10], // 0x0438 (Delegate)
    pub online_stats_login: [u8; 0x10], // 0x0448 (Delegate)
    pub online_stats_level_up: [u8; 0x10], // 0x0458 (Delegate)
    pub online_stats_stat_val_changed: [u8; 0x10], // 0x0468
    pub online_stats_orig_val_changed: [u8; 0x10], // 0x0478
    pub online_store_currency_changed: [u8; 0x10], // 0x0488
    pub online_inventory_refreshed: [u8; 0x10], // 0x0498
    
    pub current_match_level_up_results: TArray<[u8; 0x20]>, // 0x04A8 (FLevelUpResult)
    pub online_account: [u8; 0xB8],     // 0x04B8 (FTBLOnlineAccount)
    
    pub death_time: f32,                // 0x0570
    pub padding_0: [u8; 0x4],           // 0x0574
    pub respawn_stats: TArray<[u8; 0x10]>, // 0x0578 (FStatEntry)
    pub respawn_special_item: i32,      // 0x0588 (FRespawnSpecialItem enum)
    pub padding_1: [u8; 0x4],           // 0x058C
    
    pub respawn_constructable_actors: TArray<*mut c_void>, // 0x0590 (AActor*)
    pub team_before_inactive: *mut c_void, // 0x05A0 (ATBLTeam*)
    pub on_next_spawn_team_changed: [u8; 0x10], // 0x05A8
    pub attach_to_projectile: *mut c_void, // 0x05B8 (AInventoryItem*)
    pub wait_for_attach_to_projectile: *mut c_void, // 0x05C0 (ATBLCharacter*)
    
    pub playfab_oss_unique_id: FUniqueNetIdRepl, // 0x05C8 (Size 0x28)
    pub platform_oss_unique_id: FUniqueNetIdRepl, // 0x05F0 (Size 0x28)
    
    pub on_global_rank_changed: [u8; 0x10], // 0x0618
    
    // Key Gameplay Stats
    pub kills: i32,                     // 0x0628
    pub deaths: i32,                    // 0x062C
    pub assists: i32,                   // 0x0630
    pub takedowns: i32,                 // 0x0634
    pub global_rank: i32,               // 0x0638
    pub player_score: i32,              // 0x063C
    pub projectiles_fired: i32,         // 0x0640
    
    pub padding_2: [u8; 0x54],          // 0x0644
    pub on_takedowns_changed: [u8; 0x10], // 0x0698
    pub next_spawn_pawn_subclass: *mut c_void, // 0x06A8 (UClass*)
    pub whitelist_assets: [u8; 0x50],   // 0x06B0 (TMap)
    
    pub party_id: FString,        // 0x0700
    pub num_in_party: i32,              // 0x0710
    pub padding_3: [u8; 0x4],           // 0x0714
    
    pub on_kills_changed: [u8; 0x10],   // 0x0718
    pub on_deaths_changed: [u8; 0x10],  // 0x0728
    pub padding_4: [u8; 0x18],          // 0x0738
    
    pub down_causing_players: TArray<*mut c_void>, // 0x0750 (TWeakObjectPtr)
    pub score_changed: [u8; 0x10],      // 0x0760
    pub padding_5: [u8; 0x70],          // 0x0770
    
    pub on_score_event: [u8; 0x10],     // 0x07E0
    pub cached_mesh_class: *mut c_void, // 0x07F0
    pub faction_cached: u8,             // 0x07F8
    pub padding_6: [u8; 0x7],           // 0x07F9
    pub cached_mesh: *mut c_void,       // 0x0800 (USkeletalMesh*)
    pub cached_mesh_morph_targets: [u8; 0x50], // 0x0808 (TMap)
    pub cached_gore_head_meshes: TArray<*mut c_void>, // 0x0858
    
    pub wants_online_load: bool,        // 0x0868
    pub bot_selected_assets_type: u8,   // 0x0869
    pub padding_7: [u8; 0x6],           // 0x086A
    pub bot_customization_query: [u8; 0x10], // 0x0870
    pub game_sparks_user_id: FString, // 0x0880
    pub on_team_changed: [u8; 0x10],    // 0x0890
    
    pub has_been_auto_balanced: bool,   // 0x08A0
    pub padding_8: [u8; 0x7],           // 0x08A1
    pub death_recap: [u8; 0x78],        // 0x08A8
    pub customization_uploaded: bool,   // 0x0920
    pub padding_9: [u8; 0x7],           // 0x0921
    
    pub dead_characters: [u8; 0x50],    // 0x0928 (TMap)
    pub dead_character_id: u8,          // 0x0978
    pub padding_10: [u8; 0x7],          // 0x0979
    pub on_platform_changed: [u8; 0x10], // 0x0980
    pub padding_11: [u8; 0x1],          // 0x0990
    
    pub is_friend: bool,                // 0x0991
    pub can_be_auto_balanced: bool,     // 0x0992
    pub is_vip: u8,                     // 0x0993
    pub equipped_personality: u8,       // 0x0994
    pub padding_12: [u8; 0x3],          // 0x0995
    pub player_score_events_by_text: TArray<[u8; 0x20]>, // 0x0998
    
    pub quitter: bool,                  // 0x09A8
    pub padding_13: [u8; 0x3],          // 0x09A9
    pub eom_commodity_reward: i32,      // 0x09AC
    pub on_eom_commodity_changed: [u8; 0x10], // 0x09B0
    pub eom_xp_reward: i32,             // 0x09C0
    pub padding_14: [u8; 0x4],          // 0x09C4
    pub on_eom_xp_changed: [u8; 0x10],  // 0x09C8
    
    pub damage_taken_events: TArray<[u8; 0x30]>, // 0x09D8
    pub last_death_recap_char: *mut c_void, // 0x09E8 (TWeakObjectPtr)
    pub padding_15: [u8; 0x18],         // 0x09F0
    
    pub actual_start_time: i32,         // 0x0A08
    pub inactive_time: i32,             // 0x0A0C
    pub customization_nickname: [u8; 0x18], // 0x0A10 (FText)
    pub show_loadout_delay: f32,        // 0x0A28
    pub padding_16: [u8; 0xC],          // 0x0A2C
    
    pub time_since_last_out_combat: f32, // 0x0A38
    pub out_of_combat_time_rem: f32,    // 0x0A3C
    pub next_spawn_team: *mut c_void,   // 0x0A40 (ATBLTeam*)
    pub team: *mut c_void,              // 0x0A48 (ATBLTeam*)
    pub faction: u8,                    // 0x0A50 (EFaction enum)
    pub is_npc: bool,                   // 0x0A51
    pub is_player_custom_bot: bool,     // 0x0A52
    pub padding_17: [u8; 0x5],          // 0x0A53
    
    pub character_class: *mut c_void,   // 0x0A58 (UClass*)
    pub character: *mut c_void,         // 0x0A60 (ATBLCharacter*)
    pub localizable_player_name: FText, // 0x0A68 (FText) [u8; 0x18]
    pub presets: [u8; 0x48],            // 0x0A80 (FReplCustomizationPresetMapping)
    pub must_set_loadout: bool,         // 0x0AC8
    pub padding_18: [u8; 0x3],          // 0x0AC9
    
    pub agatha_color: [f32; 4],         // 0x0ACC (FLinearColor)
    pub mason_color: [f32; 4],          // 0x0ADC
    pub tenosia_color: [f32; 4],        // 0x0AEC
    pub spectator_color: [f32; 4],      // 0x0AFC
    pub padding_19: [u8; 0x4],          // 0x0B0C
    
    pub next_spawn_loadout: [u8; 0x38], // 0x0B10 (FLoadout)
    pub is_next_spawn_overridden: bool, // 0x0B48
    pub padding_20: [u8; 0x3],          // 0x0B49
    pub total_team_damage: f32,         // 0x0B4C
    pub total_idle_time: f32,           // 0x0B50
    pub padding_21: [u8; 0x4],          // 0x0B54
    pub instigated_votes: TArray<[u8; 0x20]>, // 0x0B58
    pub last_voted_time: f32,           // 0x0B68
    pub client_platform: u8,            // 0x0B6C
    pub padding_22: [u8; 0xB],          // 0x0B6D
    pub friends_list: TArray<*mut ATBLPlayerState>, // 0x0B78
} // Size: 0xB88



/// Character

#[repr(C)]
pub struct APawn {
    // 0x0000 - 0x0260: Inherited from AActor
    pub base_actor: [u8; 0x260], 

    // 0x0260: Bitfields (3 bits used)
    // C++: uint8 bUseControllerRotationPitch : 1;
    // C++: uint8 bUseControllerRotationYaw   : 1;
    // C++: uint8 bUseControllerRotationRoll  : 1;
    pub controller_rotation_bits: u8, 

    pub padding_0: [u8; 0x3],          // 0x0261 (size: 0x3)
    
    pub b_enabled_use_controller_rotation_pitch: bool, // 0x0264
    pub b_enabled_use_controller_rotation_yaw: bool,   // 0x0265
    pub b_enabled_use_controller_rotation_roll: bool,  // 0x0266
    
    pub padding_1: [u8; 0xF1],         // 0x0267 (size: 0xF1)
    
    pub b_can_affect_navigation_generation: u8, // 0x0358
    pub padding_2: [u8; 0x3],          // 0x0359
    
    pub base_eye_height: f32,          // 0x035C
    
    pub auto_possess_player: u8,       // 0x0360 (TEnumAsByte)
    pub auto_possess_ai: u8,           // 0x0361 (EAutoPossessAI)
    pub remote_view_pitch: u8,         // 0x0362
    
    pub padding_3: [u8; 0x5],          // 0x0363
    
    pub ai_controller_class: *mut c_void, // 0x0368 (TSubclassOf<AController>)
    pub player_state: *mut APlayerState,        // 0x0370 (APlayerState*)
    
    pub padding_4: [u8; 0x8],          // 0x0378
    
    pub last_hit_by: *mut c_void,      // 0x0380 (AController*)
    pub controller: *mut c_void,        // 0x0388 (AController*)
    
    pub padding_5: [u8; 0x4],          // 0x0390
    
    pub control_input_vector: FVector,      // 0x0394 (size: 0xC)
    pub last_control_input_vector: FVector, // 0x03A0 (size: 0xC)
} // Size: 0x3B0

impl APawn {
    /// Helper to check bUseControllerRotationPitch (Bit 0)
    pub fn use_controller_rotation_pitch(&self) -> bool {
        (self.controller_rotation_bits & (1 << 0)) != 0
    }

    /// Helper to check bUseControllerRotationYaw (Bit 1)
    pub fn use_controller_rotation_yaw(&self) -> bool {
        (self.controller_rotation_bits & (1 << 1)) != 0
    }

    /// Helper to check bUseControllerRotationRoll (Bit 2)
    pub fn use_controller_rotation_roll(&self) -> bool {
        (self.controller_rotation_bits & (1 << 2)) != 0
    }
}

bitflags! {
    /// Character movement bitfield 1 (offset 0x0460)
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CharacterMovementFlags1: u8 {
        const IS_CROUCHED = 1 << 0;
        const PROXY_IS_JUMP_FORCE_APPLIED = 1 << 1;
        const PRESSED_JUMP = 1 << 2;
        const CLIENT_UPDATING = 1 << 3;
        const CLIENT_WAS_FALLING = 1 << 4;
        const CLIENT_RESIMULATE_ROOT_MOTION = 1 << 5;
        const CLIENT_RESIMULATE_ROOT_MOTION_SOURCES = 1 << 6;
        const SIM_GRAVITY_DISABLED = 1 << 7;
    }
}

bitflags! {
    /// Character movement bitfield 2 (offset 0x0461)
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CharacterMovementFlags2: u8 {
        const CLIENT_CHECK_ENCROACHMENT_ON_NET_UPDATE = 1 << 0;
        const SERVER_MOVE_IGNORE_ROOT_MOTION = 1 << 1;
        const WAS_JUMPING = 1 << 2;
    }
}

#[repr(C)]
pub struct ACharacter {
    // 0x0000 - 0x03B0: Inherited from APawn
    pub base_pawn: APawn, 

    pub mesh: *mut c_void,               // 0x03B0 (USkeletalMeshComponent*)
    pub character_movement: *mut c_void, // 0x03B8 (UCharacterMovementComponent*)
    pub capsule_component: *mut c_void,  // 0x03C0 (UCapsuleComponent*)
    
    pub based_movement: [u8; 0x30],      // 0x03C8 (FBasedMovementInfo)
    pub replicated_based_movement: [u8; 0x30], // 0x03F8 (FBasedMovementInfo)
    
    pub padding_0: [u8; 0x4],            // 0x0428
    pub anim_root_motion_translation_scale: f32, // 0x042C
    pub base_translation_offset: FVector, // 0x0430 (size: 0xC)
    pub padding_1: [u8; 0x4],            // 0x043C
    pub base_rotation_offset: [u8; 0x10], //FQuat,     // 0x0440 (size: 0x10)
    
    pub replicated_server_last_transform_update_timestamp: f32, // 0x0450
    pub replay_last_transform_update_timestamp: f32,           // 0x0454
    
    pub replicated_movement_mode: u8,    // 0x0458
    pub b_in_base_replication: bool,     // 0x0459
    pub padding_2: [u8; 0x2],            // 0x045A
    pub crouched_eye_height: f32,        // 0x045C

    pub movement_flags_1: CharacterMovementFlags1, // 0x0460
    pub movement_flags_2: CharacterMovementFlags2, // 0x0461
    
    pub padding_3: [u8; 0x2],            // 0x0462
    pub jump_key_hold_time: f32,         // 0x0464
    pub jump_force_time_remaining: f32,  // 0x0468
    pub proxy_jump_force_started_time: f32, // 0x046C
    pub jump_max_hold_time: f32,         // 0x0470
    pub jump_max_count: i32,             // 0x0474
    pub jump_current_count: i32,         // 0x0478
    
    pub padding_4: [u8; 0x4],            // 0x047C
    pub on_reached_jump_apex: [u8; 0x10], // 0x0480 (Delegate)
    pub padding_5: [u8; 0x10],           // 0x0490
    
    pub movement_mode_changed_delegate: [u8; 0x10], // 0x04A0
    pub on_character_movement_updated: [u8; 0x10],  // 0x04B0
    
    pub saved_root_motion: [u8; 0x38],    // 0x04C0 (FRootMotionSourceGroup)
    pub padding_6: [u8; 0x8],            // 0x04F8
    pub client_root_motion_params: [u8; 0x40], // 0x0500 (FRootMotionMovementParams)
    pub root_motion_rep_moves: [u8; 0x10], // 0x0540 (TArray)
    pub rep_root_motion: [u8; 0x98],      // 0x0550 (FRepRootMotionMontage)
    
    pub final_padding: [u8; 0x8],         // 0x05E8 (Aligns to size 0x5F0)
} // Size: 0x5F0

impl ACharacter {
    // Offset 0x0460 Helpers
    pub fn is_crouched(&self) -> bool { self.movement_flags_1.contains(CharacterMovementFlags1::IS_CROUCHED) }
    pub fn proxy_is_jump_force_applied(&self) -> bool { self.movement_flags_1.contains(CharacterMovementFlags1::PROXY_IS_JUMP_FORCE_APPLIED) }
    pub fn pressed_jump(&self) -> bool { self.movement_flags_1.contains(CharacterMovementFlags1::PRESSED_JUMP) }
    pub fn client_updating(&self) -> bool { self.movement_flags_1.contains(CharacterMovementFlags1::CLIENT_UPDATING) }
    pub fn client_was_falling(&self) -> bool { self.movement_flags_1.contains(CharacterMovementFlags1::CLIENT_WAS_FALLING) }
    pub fn sim_gravity_disabled(&self) -> bool { self.movement_flags_1.contains(CharacterMovementFlags1::SIM_GRAVITY_DISABLED) }

    // Offset 0x0461 Helpers
    pub fn client_check_encroachment(&self) -> bool { self.movement_flags_2.contains(CharacterMovementFlags2::CLIENT_CHECK_ENCROACHMENT_ON_NET_UPDATE) }
    pub fn server_move_ignore_root_motion(&self) -> bool { self.movement_flags_2.contains(CharacterMovementFlags2::SERVER_MOVE_IGNORE_ROOT_MOTION) }
    pub fn was_jumping(&self) -> bool { self.movement_flags_2.contains(CharacterMovementFlags2::WAS_JUMPING) }
}


#[repr(C)]
pub struct ATBLCharacterBase {
    // 0x0000 - 0x05F0: Inherited from ACharacter
    pub base_character: ACharacter,

    pub significance_state: [u8; 0x28],       // 0x05F0 (FCharacterSignificanceState)
    pub movement_base: *mut c_void,          // 0x0618 (UTBLCharacterMovementBaseComponent*)
    pub view_distance: f32,                  // 0x0620
    pub padding_0: [u8; 0x4],                // 0x0624
    
    pub on_recently_rendered_delegate: [u8; 0x10], // 0x0628 (Delegate)
    pub b_was_recently_rendered: bool,       // 0x0638
    pub padding_1: [u8; 0x3],                // 0x0639
    
    pub last_anim_update_rate: i32,          // 0x063C
    pub padding_2: [u8; 0x8],                // 0x0640
    
    // State Flags (Individual booleans, not bitfields in this specific class)
    pub b_root_motion_active: bool,          // 0x0648
    pub b_montage_active: bool,              // 0x0649
    pub b_weapon_tracers_active: bool,       // 0x064A
    pub b_waiting_for_anim_notify: bool,     // 0x064B
    pub b_ignore_face_rotation: bool,        // 0x064C
    pub b_is_idle_animation: bool,           // 0x064D
    pub b_is_carryable_npc: bool,            // 0x064E
    
    pub padding_3: [u8; 0x1],                // 0x064F
    pub last_carry_time: f32,                // 0x0650
    pub mesh_visibility_flag: u8,            // 0x0654 (EMeshVisibilityFlag)
    pub padding_4: [u8; 0x3],                // 0x0655
    
    pub hidden_anim_update_max_distance: f32, // 0x0658
    pub b_kill_upon_landing: bool,           // 0x065C
    pub padding_5: [u8; 0x3],                // 0x065D
    
    // Fall Damage Logic
    pub fall_damage_starting_speed: f32,     // 0x0660
    pub padding_6: [u8; 0x4],                // 0x0664
    pub fall_damage_source: *mut c_void,     // 0x0668 (UDamageSource*)
    pub falling_damage_phys_mat_multipliers: [u8; 0x10], // 0x0670 (TArray)
    
    pub skip_client_optimizations: bool,     // 0x0680
    pub b_placed_in_world: bool,             // 0x0681
    pub padding_7: [u8; 0x6],                // 0x0682
    
    pub on_root_transform_updated: [u8; 0x10], // 0x0688 (Delegate)
    pub padding_8: [u8; 0x4],                // 0x0698
    
    pub fall_damage_scale: f32,              // 0x069C
    pub default_fall_damage_scale: f32,      // 0x06A0
    
    pub final_padding: [u8; 0xC],            // 0x06A4 to 0x06B0
} // Size: 0x6B0


#[repr(C)]
pub struct ATBLCharacter {
    // 0x0000 - 0x06B0: Inherited from ATBLCharacterBase
    pub base_tbl_character_base: ATBLCharacterBase,

    pub display_info: [u8; 0x78],           // 0x06E0
    pub audio_class_type: u8,               // 0x0758
    pub pawn_class_type: u8,                // 0x0759
    pub padding_0: [u8; 0x2],               // 0x075A
    pub online_xp_stat_name: [u8; 0x8],     // 0x075C (FName)
    pub padding_1: [u8; 0x4],               // 0x0764
    pub class_progression_spec: *mut c_void, // 0x0768
    pub abilities: *mut c_void,             // 0x0770
    pub stats: *mut c_void,                 // 0x0778
    pub headlook_component: *mut c_void,    // 0x0780
    pub replicated_server_frame: u8,        // 0x0788
    pub padding_2: [u8; 0x7],               // 0x0789
    pub last_replicated_based_movement: [u8; 0x30], // 0x0790
    pub padding_3: [u8; 0x14],              // 0x07C0
    pub random_seed: i32,                   // 0x07D4
    pub lock_mesh_rotation_state: [u8; 0x18], // 0x07D8
    pub previous_blend_in_time: f32,        // 0x07F0
    pub remote_view_yaw: u8,                // 0x07F4
    pub b_use_remote_view_yaw: bool,        // 0x07F5
    pub b_cinematic_restricted_control: u8, // 0x07F6
    pub padding_4: [u8; 0x1],               // 0x07F7
    pub cinematic_lock_angle: f32,          // 0x07F8
    pub cinematic_lock_id: i32,             // 0x07FC
    pub special_item_replicated: [u8; 0x2], // 0x0800
    pub b_should_character_be_hidden: bool, // 0x0802
    pub padding_5: [u8; 0x5],               // 0x0803
    pub being_revived_by: *mut c_void,      // 0x0808 (ATBLPlayerState*)
    pub on_being_revived_delegate: [u8; 0x10], // 0x0810
    pub mesh_1p: *mut c_void,               // 0x0820
    pub follow_mesh_component: *mut c_void, // 0x0828
    pub pushing_component: *mut c_void,      // 0x0830
    pub customization_component: *mut c_void, // 0x0838
    pub padding_6: [u8; 0x8],               // 0x0840
    pub camera_1p: *mut c_void,             // 0x0848
    pub camera_3p: *mut c_void,             // 0x0850
    pub camera_1p_socket: [u8; 0x8],        // 0x0858 (FName)
    pub camera_3p_socket: [u8; 0x8],        // 0x0860 (FName)
    pub camera_offset_1p: FVector,          // 0x0868
    pub camera_1p_blend_params: [u8; 0x3C], // 0x0874
    pub prediction_state: [u8; 0x70],       // 0x08B0
    pub character_subclasses: [u8; 0x10],   // 0x0920 (TArray)
    pub loadout_selection: *mut c_void,     // 0x0930
    pub override_loadout_selection: *mut c_void, // 0x0938
    pub equipped_carryable_on_spawn: *mut c_void, // 0x0940
    pub chicken_class: [u8; 0x28],          // 0x0948 (TSoftClassPtr)
    pub base_turn_rate: f32,                // 0x0970
    pub base_look_up_rate: f32,             // 0x0974
    pub attack_from_behind_angle: f32,      // 0x0978
    pub object_highlight_max_distance: f32, // 0x097C
    pub combat_state_component: *mut c_void, // 0x0980
    pub combat_state_synchronization: *mut c_void, // 0x0988
    pub combat_state_queue: *mut c_void,     // 0x0990
    pub combat_state_set: *mut c_void,       // 0x0998
    pub conditions_component: *mut c_void,  // 0x09A0
    pub interactable_component: *mut c_void, // 0x09A8
    pub movement_modifiers: *mut c_void,    // 0x09B0
    pub perks_component: *mut c_void,       // 0x09B8
    pub last_weapon_hit_by: u32,            // 0x09C0 (TWeakObjectPtr handle)
    pub last_character_hit_by: u32,         // 0x09C8 (TWeakObjectPtr handle)
    pub weapon_applied_bleed: u32,          // 0x09D0 (TWeakObjectPtr handle)

    // Delegates 0x09D8 - 0x0CA8 (Approx 46 delegates * 0x10)
    pub delegates: [u8; 0x2E0],             // 0x09D8

    pub hit_actor_with_weapon_infos: [u8; 0x10], // 0x0CB8
    pub head_shot_actors: [u8; 0x10],            // 0x0CC8
    pub arm_hits: [u8; 0x50],                    // 0x0CD8 (TMap)
    pub client_tracer_hit_count: i32,            // 0x0D28
    pub padding_7: [u8; 0x4],                    // 0x0D2C
    pub client_tracer_hit_bones: [u8; 0x10],     // 0x0D30
    pub client_tracer_hit_components: [u8; 0x10], // 0x0D40
    pub spectating_pawn: *mut c_void,            // 0x0D50
    pub b_has_alt_attack: bool,                  // 0x0D58
    pub padding_8: [u8; 0x3],                    // 0x0D59
    pub third_person_camera_params: [u8; 0x60],  // 0x0D5C
    pub current_camera_params: [u8; 0x60],       // 0x0DBC
    pub mount_camera_interp_speed: f32,          // 0x0E1C
    pub movement_debugger: *mut c_void,          // 0x0E20
    pub sk_mesh_3p: *mut c_void,                 // 0x0E28
    pub sight_cache_map: [u8; 0x50],             // 0x0E30
    pub padding_9: [u8; 0x10],                   // 0x0E80
    pub gore_head_rotation_offset: [f32; 3],     // 0x0E90 (FRotator)
    pub bitfield_0e9c: u8,                       // 0x0E9C (bSpawnedGoreHead, bIsForCustomizationMenu)
    pub b_customization_components_spawned: u8,  // 0x0E9D
    pub padding_10: [u8; 0x2],                   // 0x0E9E
    pub customization_context: [u8; 0x58],       // 0x0EA0
    pub default_mesh_1p: *mut c_void,            // 0x0EF8
    pub mesh_1p_location: FVector,               // 0x0F00
    pub mesh_1p_rotation: [f32; 3],              // 0x0F0C (FRotator)
    pub first_possessed_time: f32,               // 0x0F18
    pub b_has_attacked_successfully: bool,       // 0x0F1C
    pub padding_11: [u8; 0x3],                   // 0x0F1D
    pub gamepad_use_start_time: f32,             // 0x0F20
    pub gamepad_use_pressed_count: i32,          // 0x0F24
    pub debug_projectile: *mut c_void,           // 0x0F28
    pub debug_fake_client_projectile: *mut c_void, // 0x0F30
    pub debug_dropped_item: *mut c_void,         // 0x0F38
    pub b_debug_ignore_ai: bool,                 // 0x0F40
    pub b_is_cinematics_relevant: bool,          // 0x0F41
    pub padding_12: [u8; 0x6],                   // 0x0F42
    pub spawned_at_spawner: *mut c_void,         // 0x0F48
    pub spawned_at_spawn_comp: *mut c_void,      // 0x0F50
    pub spawner_position: i32,                   // 0x0F58
    pub b_prevent_forward_spawn: bool,           // 0x0F5C
    pub padding_13: [u8; 0x3],                   // 0x0F5D
    pub attached_items: [u8; 0x10],              // 0x0F60 (TArray)
    pub lock_mesh_blend_in_params: [u8; 0x54],   // 0x0F70
    pub lock_mesh_blend_out_params: [u8; 0x54],  // 0x0FC4
    pub cinematic_control_delegates: [u8; 0x20], // 0x1018
    pub mount_root_blend_params: [u8; 0x68],     // 0x1038
    pub mount_control_blend_params: [u8; 0x24],  // 0x10A0
    pub padding_14: [u8; 0x4],                   // 0x10C4
    pub vo_friendly_damage_sources: [u8; 0x10],  // 0x10C8
    pub auto_vo_negative_damage_sources: [u8; 0x10], // 0x10D8
    pub b_helmet_knocked_off: bool,              // 0x10E8
    pub dead_character_id: u8,                   // 0x10E9
    pub padding_15: [u8; 0x6],                   // 0x10EA
    pub auto_vo_probability: [u8; 0x50],         // 0x10F0 (TMap)
    pub tutorial_loc_blend_params: [u8; 0x3C],   // 0x1140
    pub padding_16: [u8; 0x4],                   // 0x117C
    pub tutorial_look_at_params: [u8; 0x20],     // 0x1180
    pub horse_look_at_params: [u8; 0x18],        // 0x11A0
    pub horse_bump_camera_params: [u8; 0x18],    // 0x11B8
    pub attached_ragdoll: [u8; 0x30],            // 0x11D0
    pub attach_ragdoll_blend_params: [u8; 0x3C], // 0x1200
    pub padding_17: [u8; 0x4],                   // 0x123C
    pub special_item_ability: *mut c_void,       // 0x1240
    pub on_special_ability_set: [u8; 0x10],      // 0x1248
    pub last_parry_event: [u8; 0x70],            // 0x1258
    pub last_fired_projectile: *mut c_void,      // 0x12C8
    pub debug_movement_replication: [u8; 0x1C],  // 0x12D0
    pub padding_18: [u8; 0x4],                   // 0x12EC
    pub knockdown_off_horse: *mut c_void,        // 0x12F0
    pub pickup_cooldown: f32,                    // 0x12F8
    pub padding_19: [u8; 0x4],                   // 0x12FC
    pub time_to_vo_expiry: f32,                  // 0x1300
    pub allowed_vos_within_expiry: i32,          // 0x1304
    pub vo_cooldown: f32,                        // 0x1308
    pub battle_cry_vo_cooldown: f32,             // 0x130C
    pub vo_queue_emote_window: f32,              // 0x1310
    pub padding_20: [u8; 0x4],                   // 0x1314
    pub cached_parry_components: [u8; 0x10],     // 0x1318
    pub closest_interactable: *mut c_void,       // 0x1328
    pub closest_stats_component: *mut c_void,     // 0x1330
    pub closest_fire_source: *mut c_void,        // 0x1338
    pub useable_actor_interactable: *mut c_void, // 0x1340
    pub interactable_settings: [u8; 0x20],       // 0x1348
    pub gamepad_dismount_start_time: f32,        // 0x1368
    pub padding_21: [u8; 0x4],                   // 0x136C
    pub inventory: *mut c_void,                  // 0x1370
    pub b_is_dying: u8,                          // 0x1378
    pub padding_22: [u8; 0x3],                   // 0x1379
    pub death_time: f32,                         // 0x137C
    pub death_anim_length: f32,                  // 0x1380
    pub death_anim_impulse: f32,                 // 0x1384
    pub b_should_crouch_override_recovery: bool, // 0x1388
    pub b_should_play_death_anim: bool,          // 0x1389
    pub padding_23: [u8; 0x2],                   // 0x138A
    pub death_montage_name: [u8; 0x8],           // 0x138C
    pub death_section_1p: [u8; 0x8],             // 0x1394
    pub death_section_3p: [u8; 0x8],             // 0x139C
    pub padding_24: [u8; 0x4],                   // 0x13A4
    pub audio_component: *mut c_void,            // 0x13A8
    pub camera_3p_rotation: [u8; 0x30],          // 0x13B0 (FTransform)
    pub ragdoll_start_time: f32,                 // 0x13E0
    pub b_ragdoll_triggered: bool,               // 0x13E4
    pub b_ragdoll_custom_tick: bool,             // 0x13E5
    pub padding_25: [u8; 0x7],                   // 0x13E6
    pub b_is_character_paused: bool,             // 0x13ED
    pub padding_26: [u8; 0x2],                   // 0x13EE
    pub paused_components: [u8; 0x10],           // 0x13F0
    pub paused_actors: [u8; 0x10],               // 0x1400
    pub paused_actor_lifespans: [u8; 0x10],      // 0x1410
    pub paused_timer_handles: [u8; 0x10],        // 0x1420
    pub paused_physics: [u8; 0x10],              // 0x1430
    pub b_has_random_seed: bool,                 // 0x1440
    pub b_did_push_respawn_state: bool,          // 0x1441
    pub b_tap_out_held: bool,                    // 0x1442
    pub padding_27: [u8; 0x1],                   // 0x1443
    pub attached_smoothing: [u8; 0xC],           // 0x1444
    pub prev_is_playing_root_motion: bool,       // 0x1450
    pub auto_possess_ai_faction: u8,             // 0x1451
    pub padding_28: [u8; 0x6],                   // 0x1452
    pub ai_behavior_tree: *mut c_void,           // 0x1458
    pub ai_flags: [u8; 0x6],                     // 0x1460 (6 booleans)
    pub padding_29: [u8; 0x2],                   // 0x1466
    pub fell_out_of_world_time: f32,             // 0x1468
    pub b_count_kills: bool,                     // 0x146C
    pub padding_30: [u8; 0x3],                   // 0x146D
    pub constructable_actors: [u8; 0x10],        // 0x1470
    pub padding_31: [u8; 0x8],                   // 0x1480
    pub last_controller: *mut c_void,            // 0x1488
    pub last_player_controller: *mut c_void,     // 0x1490
    pub last_player_state: *mut c_void,          // 0x1498
    pub vo_tracker: [u8; 0x30],                  // 0x14A0
    pub lip_sync_curve: *mut c_void,             // 0x14D0
    pub current_auto_vo: u8,                     // 0x14D8
    pub padding_32: [u8; 0x7],                   // 0x14D9
    pub death_friendly_vo: [u8; 0x10],           // 0x14E0
    pub alive_friendly_vo: [u8; 0x10],           // 0x14F0
    pub padding_33: [u8; 0x60],                  // 0x1500
    pub cached_interactable_held: *mut c_void,   // 0x1560
    pub conditions_applied_map: [u8; 0x50],      // 0x1568
    pub padding_34: [u8; 0x38],                  // 0x15B8
    pub auto_vo_distance_map: [u8; 0x50],        // 0x15F0
    pub capture_point: u32,                      // 0x1640 (TWeakObjectPtr)
    pub padding_35: [u8; 0x18],                  // 0x1648
    pub wait_for_attach_time: f32,               // 0x1660
    pub padding_36: [u8; 0x4],                   // 0x1664
    pub previous_team: *mut c_void,              // 0x1668
    pub b_is_initial_autorun: bool,              // 0x1670
    pub padding_37: [u8; 0x7],                   // 0x1671
    pub particle_prepopulate_map: [u8; 0x50],    // 0x1678
    pub b_player_needs_healing: bool,            // 0x16C8
    pub padding_38: [u8; 0x7],                   // 0x16C9
    pub healing_item_types: [u8; 0x10],          // 0x16D0
    pub healing_special_item_types: [u8; 0x10],  // 0x16E0
    pub b_last_able_to_heal_check: bool,         // 0x16F0
    pub padding_39: [u8; 0x3],                   // 0x16F1
    pub needs_healing_threshold: f32,            // 0x16F4
    pub final_padding: [u8; 0x18],               // Aligns to 0x1710
} // Size: 0x1710
