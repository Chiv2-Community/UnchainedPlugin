//! Native source: TBL/Source/TBL/TBLCharacter.h (shipping PDB).

use crate::chivalry2::engine::*;
use crate::chivalry2::tbl::*;
use std::os::raw::c_void;

define_pattern_resolver!(
    ATBLCharacter__OnKilled,
    ["4C 8B DC 55 41 55 41 57 49 8D AB ?? ?? ?? ?? 48 81 EC 30 03 00 00"]
);

define_pattern_resolver!(
    ATBLCharacter__OnDamageTaken,
    ["4C 8B DC 55 57 41 55 41 56 49 8D AB ?? ?? ?? ?? 48 81 EC 58 02 00 00"]
);

unsafe impl crate::chivalry2::engine::UObjectType for ATBLCharacter {
    const CLASS_NAME: &'static str = "TBLCharacter";
}

#[rust_ctables::cpp_struct]
pub struct ATBLCharacter {
    #[inherit]
    pub base_tbl_character_base: ATBLCharacterBase, // 0x0000

    _secondary_interfaces: [u8; 0x30],                     // 0x06B0
    pub display_info: [u8; 0x78],                          // 0x06E0
    pub audio_class_type: u8,                              // 0x0758
    pub pawn_class_type: u8,                               // 0x0759
    pub padding_0: [u8; 0x2],                              // 0x075A
    pub online_xp_stat_name: [u8; 0x8],                    // 0x075C
    pub padding_1: [u8; 0x4],                              // 0x0764
    pub class_progression_spec: *mut c_void,               // 0x0768
    pub abilities: *mut c_void,                            // 0x0770
    pub stats: *mut c_void,                                // 0x0778
    pub headlook_component: *mut c_void,                   // 0x0780
    pub replicated_server_frame: u8,                       // 0x0788
    pub padding_2: [u8; 0x7],                              // 0x0789
    pub last_replicated_based_movement: [u8; 0x30],        // 0x0790
    pub padding_3: [u8; 0x14],                             // 0x07C0
    pub random_seed: i32,                                  // 0x07D4
    pub lock_mesh_rotation_state: [u8; 0x18],              // 0x07D8
    pub previous_blend_in_time: f32,                       // 0x07F0
    pub remote_view_yaw: u8,                               // 0x07F4
    pub b_use_remote_view_yaw: bool,                       // 0x07F5
    pub b_cinematic_restricted_control: u8,                // 0x07F6
    pub padding_4: [u8; 0x1],                              // 0x07F7
    pub cinematic_lock_angle: f32,                         // 0x07F8
    pub cinematic_lock_id: i32,                            // 0x07FC
    pub special_item_replicated: [u8; 0x2],                // 0x0800
    pub b_should_character_be_hidden: bool,                // 0x0802
    pub padding_5: [u8; 0x5],                              // 0x0803
    pub being_revived_by: *mut ATBLPlayerState,            // 0x0808
    pub on_being_revived_delegate: [u8; 0x10],             // 0x0810
    pub mesh_1p: *mut c_void,                              // 0x0820
    pub follow_mesh_component: *mut c_void,                // 0x0828
    pub pushing_component: *mut c_void,                    // 0x0830
    pub customization_component: *mut c_void,              // 0x0838
    pub padding_6: [u8; 0x8],                              // 0x0840
    pub camera_1p: *mut c_void,                            // 0x0848
    pub camera_3p: *mut c_void,                            // 0x0850
    pub camera_1p_socket: [u8; 0x8],                       // 0x0858
    pub camera_3p_socket: [u8; 0x8],                       // 0x0860
    pub camera_offset_1p: FVector,                         // 0x0868
    pub camera_1p_blend_params: [u8; 0x3C],                // 0x0874
    pub prediction_state: [u8; 0x70],                      // 0x08B0
    pub character_subclasses: [u8; 0x10],                  // 0x0920
    pub loadout_selection: *mut c_void,                    // 0x0930
    pub override_loadout_selection: *mut c_void,           // 0x0938
    pub equipped_carryable_on_spawn: *mut c_void,          // 0x0940
    pub chicken_class: [u8; 0x28],                         // 0x0948
    pub base_turn_rate: f32,                               // 0x0970
    pub base_look_up_rate: f32,                            // 0x0974
    pub attack_from_behind_angle: f32,                     // 0x0978
    pub object_highlight_max_distance: f32,                // 0x097C
    pub combat_state_component: *mut c_void,               // 0x0980
    pub combat_state_synchronization: *mut c_void,         // 0x0988
    pub combat_state_queue: *mut c_void,                   // 0x0990
    pub combat_state_set: *mut c_void,                     // 0x0998
    pub conditions_component: *mut c_void,                 // 0x09A0
    pub interactable_component: *mut c_void,               // 0x09A8
    pub movement_modifiers: *mut c_void,                   // 0x09B0
    pub perks_component: *mut c_void,                      // 0x09B8
    pub last_weapon_hit_by: [u8; 8],                       // 0x09C0
    pub last_character_hit_by: [u8; 8],                    // 0x09C8
    pub weapon_applied_bleed: [u8; 8],                     // 0x09D0
    pub delegates: [u8; 0x2E0],                            // 0x09D8
    pub hit_actor_with_weapon_infos: [u8; 0x10],           // 0x0CB8
    pub head_shot_actors: [u8; 0x10],                      // 0x0CC8
    pub arm_hits: [u8; 0x50],                              // 0x0CD8
    pub client_tracer_hit_count: i32,                      // 0x0D28
    pub padding_7: [u8; 0x4],                              // 0x0D2C
    pub client_tracer_hit_bones: [u8; 0x10],               // 0x0D30
    pub client_tracer_hit_components: [u8; 0x10],          // 0x0D40
    pub spectating_pawn: *mut c_void,                      // 0x0D50
    pub b_has_alt_attack: bool,                            // 0x0D58
    pub padding_8: [u8; 0x3],                              // 0x0D59
    pub third_person_camera_params: [u8; 0x60],            // 0x0D5C
    pub current_camera_params: [u8; 0x60],                 // 0x0DBC
    pub mount_camera_interp_speed: f32,                    // 0x0E1C
    pub movement_debugger: *mut c_void,                    // 0x0E20
    pub sk_mesh_3p: *mut c_void,                           // 0x0E28
    pub sight_cache_map: [u8; 0x50],                       // 0x0E30
    pub padding_9: [u8; 0x10],                             // 0x0E80
    pub gore_head_rotation_offset: [f32; 3],               // 0x0E90
    pub bitfield_0e9c: u8,                                 // 0x0E9C
    pub b_customization_components_spawned: u8,            // 0x0E9D
    pub padding_10: [u8; 0x2],                             // 0x0E9E
    pub customization_context: [u8; 0x58],                 // 0x0EA0
    pub default_mesh_1p: *mut c_void,                      // 0x0EF8
    pub mesh_1p_location: FVector,                         // 0x0F00
    pub mesh_1p_rotation: [f32; 3],                        // 0x0F0C
    pub first_possessed_time: f32,                         // 0x0F18
    pub b_has_attacked_successfully: bool,                 // 0x0F1C
    pub padding_11: [u8; 0x3],                             // 0x0F1D
    pub gamepad_use_start_time: f32,                       // 0x0F20
    pub gamepad_use_pressed_count: i32,                    // 0x0F24
    pub debug_projectile: *mut c_void,                     // 0x0F28
    pub debug_fake_client_projectile: *mut c_void,         // 0x0F30
    pub debug_dropped_item: *mut c_void,                   // 0x0F38
    pub b_debug_ignore_ai: bool,                           // 0x0F40
    pub b_is_cinematics_relevant: bool,                    // 0x0F41
    pub padding_12: [u8; 0x6],                             // 0x0F42
    pub spawned_at_spawner: *mut c_void,                   // 0x0F48
    pub spawned_at_spawn_comp: *mut c_void,                // 0x0F50
    pub spawner_position: i32,                             // 0x0F58
    pub b_prevent_forward_spawn: bool,                     // 0x0F5C
    pub padding_13: [u8; 0x3],                             // 0x0F5D
    pub attached_items: [u8; 0x10],                        // 0x0F60
    pub lock_mesh_blend_in_params: [u8; 0x54],             // 0x0F70
    pub lock_mesh_blend_out_params: [u8; 0x54],            // 0x0FC4
    pub cinematic_control_delegates: [u8; 0x20],           // 0x1018
    pub mount_root_blend_params: [u8; 0x68],               // 0x1038
    pub mount_control_blend_params: [u8; 0x24],            // 0x10A0
    pub padding_14: [u8; 0x4],                             // 0x10C4
    pub vo_friendly_damage_sources: [u8; 0x10],            // 0x10C8
    pub auto_vo_negative_damage_sources: [u8; 0x10],       // 0x10D8
    pub b_helmet_knocked_off: bool,                        // 0x10E8
    pub dead_character_id: u8,                             // 0x10E9
    pub padding_15: [u8; 0x6],                             // 0x10EA
    pub auto_vo_probability: [u8; 0x50],                   // 0x10F0
    pub tutorial_loc_blend_params: [u8; 0x3C],             // 0x1140
    pub padding_16: [u8; 0x4],                             // 0x117C
    pub tutorial_look_at_params: [u8; 0x20],               // 0x1180
    pub horse_look_at_params: [u8; 0x18],                  // 0x11A0
    pub horse_bump_camera_params: [u8; 0x18],              // 0x11B8
    pub attached_ragdoll: [u8; 0x30],                      // 0x11D0
    pub attach_ragdoll_blend_params: [u8; 0x3C],           // 0x1200
    pub padding_17: [u8; 0x4],                             // 0x123C
    pub special_item_ability: *mut c_void,                 // 0x1240
    pub on_special_ability_set: [u8; 0x10],                // 0x1248
    pub last_parry_event: [u8; 0x70],                      // 0x1258
    pub last_fired_projectile: *mut c_void,                // 0x12C8
    pub debug_movement_replication: [u8; 0x1C],            // 0x12D0
    pub padding_18: [u8; 0x4],                             // 0x12EC
    pub knockdown_off_horse: *mut c_void,                  // 0x12F0
    pub pickup_cooldown: f32,                              // 0x12F8
    pub padding_19: [u8; 0x4],                             // 0x12FC
    pub time_to_vo_expiry: f32,                            // 0x1300
    pub allowed_vos_within_expiry: i32,                    // 0x1304
    pub vo_cooldown: f32,                                  // 0x1308
    pub battle_cry_vo_cooldown: f32,                       // 0x130C
    pub vo_queue_emote_window: f32,                        // 0x1310
    pub padding_20: [u8; 0x4],                             // 0x1314
    pub cached_parry_components: [u8; 0x10],               // 0x1318
    pub closest_interactable: *mut c_void,                 // 0x1328
    pub closest_stats_component: *mut c_void,              // 0x1330
    pub closest_fire_source: *mut c_void,                  // 0x1338
    pub useable_actor_interactable: *mut c_void,           // 0x1340
    pub interactable_settings: [u8; 0x20],                 // 0x1348
    pub gamepad_dismount_start_time: f32,                  // 0x1368
    pub padding_21: [u8; 0x4],                             // 0x136C
    pub inventory: *mut c_void,                            // 0x1370
    pub b_is_dying: u8,                                    // 0x1378
    pub padding_22: [u8; 0x3],                             // 0x1379
    pub death_time: f32,                                   // 0x137C
    pub death_anim_length: f32,                            // 0x1380
    pub death_anim_impulse: f32,                           // 0x1384
    pub b_should_crouch_override_recovery: bool,           // 0x1388
    pub b_should_play_death_anim: bool,                    // 0x1389
    pub padding_23: [u8; 0x2],                             // 0x138A
    pub death_montage_name: [u8; 0x8],                     // 0x138C
    pub death_section_1p: [u8; 0x8],                       // 0x1394
    pub death_section_3p: [u8; 0x8],                       // 0x139C
    pub padding_24: [u8; 0x4],                             // 0x13A4
    pub audio_component: *mut c_void,                      // 0x13A8
    pub camera_3p_rotation: [u8; 0x30],                    // 0x13B0
    pub ragdoll_start_time: f32,                           // 0x13E0
    pub b_ragdoll_triggered: bool,                         // 0x13E4
    pub b_ragdoll_custom_tick: bool,                       // 0x13E5
    pub padding_25: [u8; 0x7],                             // 0x13E6
    pub b_is_character_paused: bool,                       // 0x13ED
    pub padding_26: [u8; 0x2],                             // 0x13EE
    pub paused_components: [u8; 0x10],                     // 0x13F0
    pub paused_actors: [u8; 0x10],                         // 0x1400
    pub paused_actor_lifespans: [u8; 0x10],                // 0x1410
    pub paused_timer_handles: [u8; 0x10],                  // 0x1420
    pub paused_physics: [u8; 0x10],                        // 0x1430
    pub b_has_random_seed: bool,                           // 0x1440
    pub b_did_push_respawn_state: bool,                    // 0x1441
    pub b_tap_out_held: bool,                              // 0x1442
    pub padding_27: [u8; 0x1],                             // 0x1443
    pub attached_smoothing: [u8; 0xC],                     // 0x1444
    pub prev_is_playing_root_motion: bool,                 // 0x1450
    pub auto_possess_ai_faction: u8,                       // 0x1451
    pub padding_28: [u8; 0x6],                             // 0x1452
    pub ai_behavior_tree: *mut c_void,                     // 0x1458
    pub ai_flags: [u8; 0x6],                               // 0x1460
    pub padding_29: [u8; 0x2],                             // 0x1466
    pub fell_out_of_world_time: f32,                       // 0x1468
    pub b_count_kills: bool,                               // 0x146C
    pub padding_30: [u8; 0x3],                             // 0x146D
    pub constructable_actors: [u8; 0x10],                  // 0x1470
    pub padding_31: [u8; 0x8],                             // 0x1480
    pub last_controller: *mut AController,                 // 0x1488
    pub last_player_controller: *mut ATBLPlayerController, // 0x1490
    pub last_player_state: *mut ATBLPlayerState,           // 0x1498
    pub vo_tracker: [u8; 0x30],                            // 0x14A0
    pub lip_sync_curve: *mut c_void,                       // 0x14D0
    pub current_auto_vo: u8,                               // 0x14D8
    pub padding_32: [u8; 0x7],                             // 0x14D9
    pub death_friendly_vo: [u8; 0x10],                     // 0x14E0
    pub alive_friendly_vo: [u8; 0x10],                     // 0x14F0
    pub padding_33: [u8; 0x60],                            // 0x1500
    pub cached_interactable_held: *mut c_void,             // 0x1560
    pub conditions_applied_map: [u8; 0x50],                // 0x1568
    pub padding_34: [u8; 0x38],                            // 0x15B8
    pub auto_vo_distance_map: [u8; 0x50],                  // 0x15F0
    pub capture_point: [u8; 8],                            // 0x1640
    pub padding_35: [u8; 0x18],                            // 0x1648
    pub wait_for_attach_time: f32,                         // 0x1660
    pub padding_36: [u8; 0x4],                             // 0x1664
    pub previous_team: *mut c_void,                        // 0x1668
    pub b_is_initial_autorun: bool,                        // 0x1670
    pub padding_37: [u8; 0x7],                             // 0x1671
    pub particle_prepopulate_map: [u8; 0x50],              // 0x1678
    pub b_player_needs_healing: bool,                      // 0x16C8
    pub padding_38: [u8; 0x7],                             // 0x16C9
    pub healing_item_types: [u8; 0x10],                    // 0x16D0
    pub healing_special_item_types: [u8; 0x10],            // 0x16E0
    pub b_last_able_to_heal_check: bool,                   // 0x16F0
    pub padding_39: [u8; 0x3],                             // 0x16F1
    pub needs_healing_threshold: f32,                      // 0x16F4
    pub final_padding: [u8; 0x18],                         // 0x16F8
} // Size: 0x1710

#[unchained_macros::runtime_bindings]
impl ATBLCharacter {
    /// # Safety
    /// UObjectType::from_ptr's game-thread and lifetime requirements apply.
    pub unsafe fn last_player_state(&self) -> Option<&APlayerState> {
        APlayerState::from_field(self.last_player_state, "TBLCharacter.LastPlayerState")
    }

    #[unchained_macros::bind_resolved_func(ATBLCharacter__OnKilled)]
    pub(crate) unsafe fn on_killed(self: *mut Self, damage_event: *const FDeathDamageTakenEvent) {}

    #[unchained_macros::bind_resolved_func(ATBLCharacter__OnDamageTaken)]
    pub(crate) unsafe fn on_damage_taken(self: *mut Self, damage_event: *const FDamageTakenEvent) {}
}
