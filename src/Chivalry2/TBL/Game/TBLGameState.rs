//! Native source: TBL/Source/TBL/Game/TBLGameState.h (shipping PDB).

#![allow(non_snake_case)]
use crate::Chivalry2::Engine::*;
use crate::Chivalry2::TBL::*;
use std::os::raw::c_void;

unsafe impl crate::Chivalry2::Engine::UObjectType for ATBLGameState {
    const CLASS_NAME: &'static str = "TBLGameState";
}

pub fn get_human_player_count() -> usize {
    let world_ptr = match crate::globals().world() {
        Some(ptr) => ptr,
        None => {
            swarn!("globals.world was None; returning 0 and skipping player count scan");
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
    pub game_mode_class: *mut c_void,               // 0x0258
    pub authority_game_mode: *mut c_void,           // 0x0260
    pub spectator_class: *mut c_void,               // 0x0268
    pub player_array: TArray<*mut ATBLPlayerState>, // 0x0270
    pub b_replicated_has_begun_play: bool,          // 0x0280
    pub padding_base_0: [u8; 0x3],                  // 0x0281
    pub replicated_world_time_seconds: f32,         // 0x0284
    pub server_world_time_seconds_delta: f32,       // 0x0288
    pub server_world_time_seconds_update_freq: f32, // 0x028C
    pub padding_base_1: [u8; 0x18],                 // 0x0290 -> 0x02A8

    // --- 0x02A8 -> 0x02D8: AGameState ---
    pub match_state: [u8; 0x8],          // 0x02A8 (FName)
    pub previous_match_state: [u8; 0x8], // 0x02B0 (FName)
    pub elapsed_time: i32,               // 0x02B8
    pub padding_gamestate_0: [u8; 0x1C], // 0x02BC -> 0x02D8

    // --- 0x02D8 -> 0x0998: ATBLGameState ---
    pub teams: TArray<*mut c_void>,         // 0x02D8
    pub ffa_team: *mut c_void,              // 0x02E8
    pub neutral_team_class: *mut c_void,    // 0x02F0
    pub map_name_padding: [u8; 0x18],       // 0x02F8 (FText)
    pub gamemode_type: u8,                  // 0x0310
    pub padding_0: [u8; 0x7],               // 0x0311
    pub game_mode_name_padding: [u8; 0x18], // 0x0318 (FText)
    pub server_name: FText,                 // 0x0330
    pub platform: FString,                  // 0x0348
    pub victor: u8,                         // 0x0358
    pub padding_1: [u8; 0x7],               // 0x0359
    pub player_victor: *mut c_void,         // 0x0360
    pub final_match_duration: f32,          // 0x0368
    pub padding_2: [u8; 0x4],               // 0x036C
    pub next_map_name: FString,             // 0x0370

    // Delegates / Events (size: 0x10 each)
    pub on_match_started_padding: [u8; 0x10], // 0x0380
    pub on_match_ended_padding: [u8; 0x10],   // 0x0390
    pub on_match_won_by_padding: [u8; 0x10],  // 0x03A0

    pub padding_3: [u8; 0x18], // 0x03B0

    pub on_playing_state_begun_padding: [u8; 0x10], // 0x03C8
    pub on_player_killed_padding: [u8; 0x10],       // 0x03D8
    pub on_player_state_added_padding: [u8; 0x10],  // 0x03E8
    pub on_player_state_removed_padding: [u8; 0x10], // 0x03F8
    pub on_player_state_uid_replicated_padding: [u8; 0x10], // 0x0408
    pub on_player_state_kills_updated_padding: [u8; 0x10], // 0x0418
    pub on_character_spawned_padding: [u8; 0x10],   // 0x0428
    pub on_epic_end_game_event_padding: [u8; 0x10], // 0x0438

    pub epic_end_game_state: u8,                 // 0x0448
    pub padding_4: [u8; 0x3],                    // 0x0449
    pub stage_end_time: f32,                     // 0x044C
    pub stage_start_time: f32,                   // 0x0450
    pub padding_5: [u8; 0x4],                    // 0x0454
    pub stage_progress_list_padding: [u8; 0x10], // 0x0458 (TArray)
    pub server_time_difference: f32,             // 0x0468
    pub b_disable_player_name_plates: bool,      // 0x046C
    pub b_only_show_names_on_teammates: bool,    // 0x046D
    pub b_is_game_mode_ffa: bool,                // 0x046E
    pub b_disable_team_select: bool,             // 0x046F
    pub b_hide_hud_important_messages: bool,     // 0x0470
    pub padding_6: [u8; 0x3],                    // 0x0471
    pub min_respawn_time: f32,                   // 0x0474
    pub players_needed_to_start_early: i32,      // 0x0478
    pub min_players_to_start: i32,               // 0x047C

    pub gameplay_event_msg_class_padding: [u8; 0x28], // 0x0480 (TSoftClassPtr)

    // Packed bitfield/flags at 0x04A8
    pub performance_flags: u8, // 0x04A8 (bits for weapon tracers, network, etc)
    pub b_server_bad_frame_time: bool, // 0x04A9
    pub b_use_open_loadout: bool, // 0x04AA
    pub padding_7: [u8; 0x1],  // 0x04AB
    pub progress_bar_primary_team_index: i32, // 0x04AC

    pub class_limits_padding: [u8; 0x10],     // 0x04B0 (TArray)
    pub game_scoring_data_table: *mut c_void, // 0x04C0
    pub team_score_format_data_table: *mut c_void, // 0x04C8
    pub novelty_score_data_table: *mut c_void, // 0x04D0

    pub first_spawn_sound_padding: [u8; 0x10], // 0x04D8 (FRespawnStinger)
    pub respawn_sound_padding: [u8; 0x10],     // 0x04E8 (FRespawnStinger)
    pub objective_point_array_padding: [u8; 0x10], // 0x04F8 (TArray)
    pub most_recent_team_score_event_padding: [u8; 0x40], // 0x0508 (FTeamScoreEvent)

    pub b_using_new_spawn_system: bool,                   // 0x0548
    pub cinematic_state: u8,                              // 0x0549
    pub padding_8: [u8; 0x6],                             // 0x054A
    pub on_cinematic_state_changed_padding: [u8; 0x10],   // 0x0550
    pub defender_cinematic_sequence: *mut c_void,         // 0x0560
    pub attacker_cinematic_sequence: *mut c_void,         // 0x0568
    pub replicated_sequence_bindings_padding: [u8; 0x10], // 0x0570

    pub padding_9: [u8; 0xA0],                              // 0x0580
    pub server_cinematic_start_time: f32,                   // 0x0620
    pub padding_10: [u8; 0x4],                              // 0x0624
    pub on_match_state_changed_padding: [u8; 0x10],         // 0x0628
    pub stage_persistent_msgs_padding: [u8; 0x10],          // 0x0638 (TArray)
    pub stage_ending_music_padding: [u8; 0x10],             // 0x0648 (FAKAudioStartStopStruct)
    pub context_objective_message_padding: [u8; 0x68],      // 0x0658 (FObjectiveContextMessage)
    pub on_new_objective_msg_padding: [u8; 0x10],           // 0x06C0
    pub voting_manager_component: *mut c_void,              // 0x06D0
    pub b_disable_spawning_bots: bool,                      // 0x06D8
    pub padding_11: [u8; 0x7],                              // 0x06D9
    pub lobby_id: FString,                                  // 0x06E0
    pub on_post_match_end_time_changed_padding: [u8; 0x10], // 0x06F0
    pub ps5_match_id: FString,                              // 0x0700
    pub on_ps5_match_id_updated_padding: [u8; 0x10],        // 0x0710
    pub ps5_match_responsible_player_padding: [u8; 0x28],   // 0x0720 (FUniqueNetIdRepl)

    pub padding_12: [u8; 0x10],                   // 0x0748
    pub warmup_end_time: f32,                     // 0x0758
    pub preparing_match_end_time: f32,            // 0x075C
    pub post_match_end_time: f32,                 // 0x0760
    pub post_match_matchmaking_start_server: f32, // 0x0764
    pub b_block_vote_kicking: u8,                 // 0x0768
    pub b_is_community_server: u8,                // 0x0769
    pub padding_13: [u8; 0x2],                    // 0x076A
    pub camera_mode_override: [u8; 0x8],          // 0x076C (FName)

    pub padding_14: [u8; 0xA8],           // 0x0774
    pub b_was_notified_waiting: bool,     // 0x081C
    pub b_delay_garbage_collection: bool, // 0x081D
    pub padding_15: [u8; 0x22],           // 0x081E

    pub zone_authorities_padding: [u8; 0x10], // 0x0840 (TArray)
    pub torn_off_items_padding: [u8; 0x50],   // 0x0850 (TMap)
    pub server_torn_off_items_padding: [u8; 0x50], // 0x08A0 (TMap)
    pub padding_16: [u8; 0x50],               // 0x08F0
    pub current_players_in_parties_padding: [u8; 0x50], // 0x0940 (TMap)
    pub xp_reward_modifier: f32,              // 0x0990
    pub gold_reward_modifier: f32,            // 0x0994
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
