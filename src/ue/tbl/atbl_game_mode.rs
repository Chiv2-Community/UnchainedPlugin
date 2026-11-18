use crate::game::chivalry2::EChatType;
use crate::ue::{FString, FText, TArray};
use std::ffi::c_void;
use unchained_macros::bind_resolver_func;

// void ATBLGameMode::BroadcastLocalizedChat(const FText&, EChatType)
define_pattern_resolver!(BroadcastLocalizedChat,["48 89 74 24 10 57 48 83 EC 30 48 8B 01 41 8B F8 48 8B F2 ?? ?? ?? ?? ?? ?? 48 8B C8 48 8D"]);

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

impl ATBLGameMode {
    /// # Safety
    /// Self must be a live native game mode, text a live FText. Call on the
    /// engine thread; neither pointer is owned by this call.
    #[bind_resolver_func(self::BroadcastLocalizedChat)]
    pub unsafe fn broadcast_localized_chat(self: *mut Self, text: *const FText, chat_type: EChatType) {}
}
