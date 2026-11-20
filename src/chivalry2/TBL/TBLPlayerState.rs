//! Native source: TBL/Source/TBL/TBLPlayerState.h (shipping PDB).

#![allow(non_snake_case)]
use crate::chivalry2::Engine::*;
use crate::chivalry2::TBL::*;
use std::os::raw::c_void;

unsafe impl crate::chivalry2::Engine::UObjectType for ATBLPlayerState {
    const CLASS_NAME: &'static str = "TBLPlayerState";
}

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

#[rust_ctables::cpp_struct]
pub struct ATBLPlayerState {
    #[inherit]
    pub base: crate::chivalry2::Engine::Runtime::Engine::GameFramework::PlayerState::APlayerState, // 0x0000 (Size: 0x358 approx)
    pub _align_to_stats: [u8; 0x8], // 0x0358 -> 0x0360 (alignment)

    pub online_stats: [u8; 0xD8],          // 0x0360 (FTBLOnlineStats)
    pub online_stats_updated: [u8; 0x10],  // 0x0438 (Delegate)
    pub online_stats_login: [u8; 0x10],    // 0x0448 (Delegate)
    pub online_stats_level_up: [u8; 0x10], // 0x0458 (Delegate)
    pub online_stats_stat_val_changed: [u8; 0x10], // 0x0468
    pub online_stats_orig_val_changed: [u8; 0x10], // 0x0478
    pub online_store_currency_changed: [u8; 0x10], // 0x0488
    pub online_inventory_refreshed: [u8; 0x10], // 0x0498

    pub current_match_level_up_results: TArray<[u8; 0x20]>, // 0x04A8 (FLevelUpResult)
    pub online_account: [u8; 0xB8],                         // 0x04B8 (FTBLOnlineAccount)

    pub death_time: f32,                   // 0x0570
    pub padding_0: [u8; 0x4],              // 0x0574
    pub respawn_stats: TArray<[u8; 0x10]>, // 0x0578 (FStatEntry)
    pub respawn_special_item: i32,         // 0x0588 (FRespawnSpecialItem enum)
    pub padding_1: [u8; 0x4],              // 0x058C

    pub respawn_constructable_actors: TArray<*mut c_void>, // 0x0590 (AActor*)
    pub team_before_inactive: *mut c_void,                 // 0x05A0 (ATBLTeam*)
    pub on_next_spawn_team_changed: [u8; 0x10],            // 0x05A8
    pub attach_to_projectile: *mut c_void,                 // 0x05B8 (AInventoryItem*)
    pub wait_for_attach_to_projectile: *mut c_void,        // 0x05C0 (ATBLCharacter*)

    pub playfab_oss_unique_id: FUniqueNetIdRepl, // 0x05C8 (Size 0x28)
    pub platform_oss_unique_id: FUniqueNetIdRepl, // 0x05F0 (Size 0x28)

    pub on_global_rank_changed: [u8; 0x10], // 0x0618

    // Key Gameplay Stats
    pub kills: i32,             // 0x0628
    pub deaths: i32,            // 0x062C
    pub assists: i32,           // 0x0630
    pub takedowns: i32,         // 0x0634
    pub global_rank: i32,       // 0x0638
    pub player_score: i32,      // 0x063C
    pub projectiles_fired: i32, // 0x0640

    pub padding_2: [u8; 0x54],                 // 0x0644
    pub on_takedowns_changed: [u8; 0x10],      // 0x0698
    pub next_spawn_pawn_subclass: *mut c_void, // 0x06A8 (UClass*)
    pub whitelist_assets: [u8; 0x50],          // 0x06B0 (TMap)

    pub party_id: FString,    // 0x0700
    pub num_in_party: i32,    // 0x0710
    pub padding_3: [u8; 0x4], // 0x0714

    pub on_kills_changed: [u8; 0x10],  // 0x0718
    pub on_deaths_changed: [u8; 0x10], // 0x0728
    pub padding_4: [u8; 0x18],         // 0x0738

    pub down_causing_players: TArray<*mut c_void>, // 0x0750 (TWeakObjectPtr)
    pub score_changed: [u8; 0x10],                 // 0x0760
    pub padding_5: [u8; 0x70],                     // 0x0770

    pub on_score_event: [u8; 0x10],                   // 0x07E0
    pub cached_mesh_class: *mut c_void,               // 0x07F0
    pub faction_cached: u8,                           // 0x07F8
    pub padding_6: [u8; 0x7],                         // 0x07F9
    pub cached_mesh: *mut c_void,                     // 0x0800 (USkeletalMesh*)
    pub cached_mesh_morph_targets: [u8; 0x50],        // 0x0808 (TMap)
    pub cached_gore_head_meshes: TArray<*mut c_void>, // 0x0858

    pub wants_online_load: bool,             // 0x0868
    pub bot_selected_assets_type: u8,        // 0x0869
    pub padding_7: [u8; 0x6],                // 0x086A
    pub bot_customization_query: [u8; 0x10], // 0x0870
    pub game_sparks_user_id: FString,        // 0x0880
    pub on_team_changed: [u8; 0x10],         // 0x0890

    pub has_been_auto_balanced: bool, // 0x08A0
    pub padding_8: [u8; 0x7],         // 0x08A1
    pub death_recap: [u8; 0x78],      // 0x08A8
    pub customization_uploaded: bool, // 0x0920
    pub padding_9: [u8; 0x7],         // 0x0921

    pub dead_characters: [u8; 0x50],     // 0x0928 (TMap)
    pub dead_character_id: u8,           // 0x0978
    pub padding_10: [u8; 0x7],           // 0x0979
    pub on_platform_changed: [u8; 0x10], // 0x0980
    pub padding_11: [u8; 0x1],           // 0x0990

    pub is_friend: bool,                                 // 0x0991
    pub can_be_auto_balanced: bool,                      // 0x0992
    pub is_vip: u8,                                      // 0x0993
    pub equipped_personality: u8,                        // 0x0994
    pub padding_12: [u8; 0x3],                           // 0x0995
    pub player_score_events_by_text: TArray<[u8; 0x20]>, // 0x0998

    pub quitter: bool,                        // 0x09A8
    pub padding_13: [u8; 0x3],                // 0x09A9
    pub eom_commodity_reward: i32,            // 0x09AC
    pub on_eom_commodity_changed: [u8; 0x10], // 0x09B0
    pub eom_xp_reward: i32,                   // 0x09C0
    pub padding_14: [u8; 0x4],                // 0x09C4
    pub on_eom_xp_changed: [u8; 0x10],        // 0x09C8

    pub damage_taken_events: TArray<[u8; 0x30]>, // 0x09D8
    pub last_death_recap_char: *mut c_void,      // 0x09E8 (TWeakObjectPtr)
    pub padding_15: [u8; 0x18],                  // 0x09F0

    pub actual_start_time: i32,             // 0x0A08
    pub inactive_time: i32,                 // 0x0A0C
    pub customization_nickname: [u8; 0x18], // 0x0A10 (FText)
    pub show_loadout_delay: f32,            // 0x0A28
    pub padding_16: [u8; 0xC],              // 0x0A2C

    pub time_since_last_out_combat: f32, // 0x0A38
    pub out_of_combat_time_rem: f32,     // 0x0A3C
    pub next_spawn_team: *mut c_void,    // 0x0A40 (ATBLTeam*)
    pub team: *mut c_void,               // 0x0A48 (ATBLTeam*)
    pub faction: EFaction,               // 0x0A50
    pub is_npc: bool,                    // 0x0A51
    pub is_player_custom_bot: bool,      // 0x0A52
    pub padding_17: [u8; 0x5],           // 0x0A53

    pub character_class: *mut c_void,   // 0x0A58 (UClass*)
    pub character: *mut c_void,         // 0x0A60 (ATBLCharacter*)
    pub localizable_player_name: FText, // 0x0A68 (FText) [u8; 0x18]
    pub presets: [u8; 0x48],            // 0x0A80 (FReplCustomizationPresetMapping)
    pub must_set_loadout: bool,         // 0x0AC8
    pub padding_18: [u8; 0x3],          // 0x0AC9

    pub agatha_color: [f32; 4],    // 0x0ACC (FLinearColor)
    pub mason_color: [f32; 4],     // 0x0ADC
    pub tenosia_color: [f32; 4],   // 0x0AEC
    pub spectator_color: [f32; 4], // 0x0AFC
    pub padding_19: [u8; 0x4],     // 0x0B0C

    pub next_spawn_loadout: [u8; 0x38],       // 0x0B10 (FLoadout)
    pub is_next_spawn_overridden: bool,       // 0x0B48
    pub padding_20: [u8; 0x3],                // 0x0B49
    pub total_team_damage: f32,               // 0x0B4C
    pub total_idle_time: f32,                 // 0x0B50
    pub padding_21: [u8; 0x4],                // 0x0B54
    pub instigated_votes: TArray<[u8; 0x20]>, // 0x0B58
    pub last_voted_time: f32,                 // 0x0B68
    pub client_platform: u8,                  // 0x0B6C
    pub padding_22: [u8; 0xB],                // 0x0B6D
    pub friends_list: TArray<*mut ATBLPlayerState>, // 0x0B78
} // Size: 0xB88
