use crate::game::chivalry2::{AActor, ATBLGameState};
use crate::game::engine::{ENetMode, FActorSpawnParameters, FRotator, UNetDriver};
use crate::ue::{FString, FVector, TArray, UClass, UObject};
use std::ffi::c_void;
use unchained_macros::bind_resolver_func;

// Used to set World reference
define_pattern_resolver!(InternalGetNetMode, {
    EGS: ["40 53 48 81 EC 90 00 00 00 48 8B D9 48 8B 49 38 48 85 C9"], // EGS
    STEAM: [
        "40 57 48 81 EC 90 00 00 00 48 8B F9 48 8B",
        "40 53 48 81 EC 90 00 00 00 48 8B D9 48 8B 49 38 48 85 C9" // EGS 2.11.4
        ], // STEAM
});

// class FString* UWorld::GetMapName(class UWorld const* `this`, class FString* __return)
define_pattern_resolver!(UWorld__GetMapName, ["40 53 48 83 EC 40 48 89 6C 24 ?? 48 8B DA 48 89 7C 24"]);

// AActor * __thiscall UWorld::SpawnActor(UWorld *this,UClass *param_1,FVector *param_2,FRotator *param_3,FActorSpawnParameters *param_4)
define_pattern_resolver!(SpawnActor,["40 53 56 57 48 83 EC 70 48 8B 05 09 4B 63 02 48 33 C4 48 89 44 24 60 0F 28 1D E2 74"]);

// SAFETY: This supported native layout has its UObject prefix at offset zero.
unsafe impl crate::ue::UObjectType for UWorld {
    const CLASS_NAME: &'static str = "World";
}

#[repr(C)]
pub struct UWorld {
    // 0x0000 -> 0x0028: Inherited from UObject
    pub base: UObject,
    pub _padding_to_persistent: [u8; 0x8], // 0x0028 -> 0x0030

    // 0x0030 (size: 0x8)
    pub persistent_level: *mut c_void, // ULevel*
    // 0x0038 (size: 0x8)
    pub net_driver: *mut UNetDriver,
    // 0x0040 (size: 0x8)
    pub line_batcher: *mut c_void, // ULineBatchComponent*
    // 0x0048 (size: 0x8)
    pub persistent_line_batcher: *mut c_void, // ULineBatchComponent*
    // 0x0050 (size: 0x8)
    pub foreground_line_batcher: *mut c_void, // ULineBatchComponent*
    // 0x0058 (size: 0x8)
    pub network_manager: *mut c_void, // AGameNetworkManager*
    // 0x0060 (size: 0x8)
    pub physics_collision_handler: *mut c_void, // UPhysicsCollisionHandler*

    // 0x0068 (size: 0x10)
    pub extra_referenced_objects: TArray<*mut c_void>, // TArray<UObject*>
    // 0x0078 (size: 0x10)
    pub per_module_data_objects: TArray<*mut c_void>, // TArray<UObject*>
    // 0x0088 (size: 0x10)
    pub level_sequence_actors: TArray<*mut c_void>, // TArray<AActor*>
    // 0x0098 (size: 0x10)
    pub streaming_levels: TArray<*mut c_void>, // TArray<ULevelStreaming*>

    // 0x00A8 (size: 0x28)
    pub streaming_levels_to_consider_padding: [u8; 0x28], // FStreamingLevelsToConsider

    // 0x00D0 (size: 0x10)
    pub streaming_levels_prefix: FString,

    // 0x00E0 (size: 0x8)
    pub current_level_pending_visibility: *mut c_void, // ULevel*
    // 0x00E8 (size: 0x8)
    pub current_level_pending_invisibility: *mut c_void, // ULevel*
    // 0x00F0 (size: 0x8)
    pub demo_net_driver: *mut c_void, // UDemoNetDriver*
    // 0x00F8 (size: 0x8)
    pub my_particle_event_manager: *mut c_void, // AParticleEventManager*
    // 0x0100 (size: 0x8)
    pub default_physics_volume: *mut c_void, // APhysicsVolume*

    // 0x0108 (size: 0x16)
    pub padding_0: [u8; 0x16],
    // 0x011E (size: 0x1)
    pub are_constraints_dirty: u8,
    // 0x011F (size: 0x1)
    pub padding_1: [u8; 0x1],

    // 0x0120 (size: 0x8)
    pub navigation_system: *mut c_void, // UNavigationSystemBase*
    // 0x0128 (size: 0x8)
    pub authority_game_mode: *mut c_void, // AGameModeBase*
    // 0x0130 (size: 0x8)
    pub game_state: *mut ATBLGameState, // AGameStateBase*
    // 0x0138 (size: 0x8)
    pub ai_system: *mut c_void, // UAISystemBase*
    // 0x0140 (size: 0x8)
    pub avoidance_manager: *mut c_void, // UAvoidanceManager*

    // 0x0148 (size: 0x10)
    pub levels: TArray<*mut c_void>, // TArray<ULevel*>
    // 0x0158 (size: 0x10)
    pub level_collections_padding: [u8; 0x10], // TArray<FLevelCollection>

    // 0x0168 (size: 0x20)
    pub padding_2: [u8; 0x20],

    // 0x0188 (size: 0x8)
    pub owning_game_instance: *mut c_void, // UGameInstance*
    // 0x0190 (size: 0x10)
    pub parameter_collection_instances: TArray<*mut c_void>, // TArray<UMaterialParameterCollectionInstance*>
    // 0x01A0 (size: 0x8)
    pub canvas_for_rendering_to_target: *mut c_void, // UCanvas*
    // 0x01A8 (size: 0x8)
    pub canvas_for_draw_material: *mut c_void, // UCanvas*

    // 0x01B0 (size: 0x50)
    pub padding_3: [u8; 0x50],

    // 0x0200 (size: 0x10)
    pub components_need_eof_update: TArray<*mut c_void>, // TArray<UActorComponent*>
    // 0x0210 (size: 0x10)
    pub components_need_eof_update_game_thread: TArray<*mut c_void>, // TArray<UActorComponent*>

    // 0x0220 (size: 0x358)
    pub padding_4: [u8; 0x358],

    // 0x0578 (size: 0x8)
    pub world_composition: *mut c_void, // UWorldComposition*

    // 0x0580 (size: 0x88)
    pub padding_5: [u8; 0x88],

    // 0x0608 (size: 0x8)
    pub physics_asset_pool: *mut c_void, // UPhysicsAssetWorldPool*

    // 0x0610 (size: 0x60)
    pub psc_pool_padding: [u8; 0x60], // FWorldPSCPool

    // Final padding to match 0x738
    pub _final_padding: [u8; 0x68],
}

impl UWorld {
    /// # Safety
    /// Self must be a live native world on the engine thread.
    #[bind_resolver_func(self::InternalGetNetMode)]
    pub unsafe fn internal_get_net_mode(self: *const Self) -> ENetMode {}

    /// UWorld::GetMapName() const, with explicit hidden FString return storage.
    ///
    /// # Safety
    /// Self must be a live native world on the engine thread and output writable
    /// native FString storage. The caller owns the resulting string.
    #[bind_resolver_func(self::UWorld__GetMapName)]
    pub unsafe fn get_map_name<'a>(self: *const Self, output: &'a mut FString) -> &'a mut FString {}

    /// UWorld::SpawnActor(UClass*, FVector const*, FRotator const*, ...).
    ///
    /// # Safety
    /// Call on the engine thread with a live world, compatible actor class and
    /// valid native parameter storage. The world owns any returned actor.
    #[bind_resolver_func(self::SpawnActor)]
    pub unsafe fn spawn_actor(self: *mut Self, class: *mut UClass, position: *const FVector, rotation: *const FRotator, parameters: *const FActorSpawnParameters) -> *mut AActor {}
}
