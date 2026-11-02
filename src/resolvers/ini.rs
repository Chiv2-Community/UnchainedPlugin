use crate::{
    config_overrides::{self, route_config},
    game::{chivalry2::ATBLGameMode, engine::UWorld},
    tools::hook_globals::{cli_args, globals_initialized},
    ue::{
        AGameModeBase, FConfigCacheIni, FConfigFile, FProperty, FString, FWorldContext, UClass,
        UEngine, UObject, UPendingNetGame, UTBLGameInstance, FURL,
    },
};

// FConfigFile::FConfigFile(const FConfigFile&) — call-only binding, not an active detour.
define_pattern_resolver!(ConfigFileCopy, [
    "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 20 33 ED C7 41 2C 80 00 00 00 48 89 29 0F 57 C0 48 89 69 08 48 8B F2"
]);
CREATE_HOOK!(ConfigFileCopy, CALLED, NONE, (), (dest: *mut FConfigFile, source: *const FConfigFile), {});

// FConfigFile::~FConfigFile — call-only binding.
define_pattern_resolver!(ConfigFileDestroy, [
    "48 89 5C 24 08 48 89 74 24 10 57 48 83 EC 20 48 8B 05 ?? ?? ?? ?? 48 8B D9 48 63 0D ?? ?? ?? ?? 48 C1 E1 04 48 03 C8 48 3B C1 74 33"
]);
CREATE_HOOK!(ConfigFileDestroy, CALLED, NONE, (), (file: *mut FConfigFile), {});

// FConfigFile::AddDynamicLayerToHeirarchy — UE's spelling; call-only binding.
define_pattern_resolver!(ConfigFileAddLayer, [
    "40 55 48 8B EC 48 83 EC 70 48 89 B4 24 90 00 00 00 48 8B F2 48 89 7C 24 68 48 8B F9 4C 89 74 24 60 45 33 F6 4C 89 75 B0 4C 89 75 B8"
]);
CREATE_HOOK!(ConfigFileAddLayer, CALLED, NONE, (), (file: *mut FConfigFile, filename: *const FString), {});

// FConfigFile::GetInt — read the explicit startup slot from the unlayered baseline.
define_pattern_resolver!(ConfigFileGetInt, [
    "40 53 48 83 EC 30 33 C0 49 8B D9 4C 8D 4C 24 20 48 89 44 24 20 48 89 44 24 28 E8 ?? ?? ?? ?? 84 C0 74 1E 83 7C 24 28 00 48 8D 0D ?? ?? ?? ?? 48 0F 45 4C 24 20 FF 15 ?? ?? ?? ?? 89 03"
]);
CREATE_HOOK!(ConfigFileGetInt, CALLED, NONE, bool, (
    file: *const FConfigFile, section: *const u16, key: *const u16, value: *mut i32
), false);

// FConfigCacheIni::SetInt — preserve only native rotation state, never conditional settings.
define_pattern_resolver!(ConfigCacheSetInt, [
    "40 53 55 56 57 41 56 48 81 EC 40 08 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 84 24 30 08 00 00 48 8B 9C 24 90 08 00 00 4D 8B F0 48 8B EA 48 8B F1"
]);
CREATE_HOOK!(ConfigCacheSetInt, CALLED, NONE, (), (
    cache: *mut FConfigCacheIni, section: *const u16, key: *const u16, value: i32, filename: *const FString
), {});

// UObject::LoadConfig — call-only binding.
// Used only to refresh existing class defaults before destination gameplay is initialized.
// Each import reads ONE already-merged config, not a sequence of individual override INIs.
define_pattern_resolver!(LoadConfig, [
    "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 D0 FC FF FF 48 81 EC 30 04 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 10 03 00 00"
]);
CREATE_HOOK!(LoadConfig, CALLED, NONE, (), (
    object: *mut UObject, class: *mut UClass, filename: *const u16, flags: u32, property: *mut FProperty
), {});

// FConfigCacheIni::Find — intercept config lookup to return a private merged config.
define_pattern_resolver!(ConfigCacheFind, [
    "40 53 56 41 54 41 56 48 81 EC A8 01 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 84 24 90 01 00 00 8B 42 08 33 DB 85 C0 45 0F B6 E0 4C 8B F2"
]);
CREATE_HOOK!(ConfigCacheFind, { || cli_args().unchained_profile_location.is_some() }, NONE, *mut FConfigFile, (
    cache: *mut FConfigCacheIni, filename: *const FString, create: bool
), {
    let baseline = CALL_ORIGINAL!(ConfigCacheFind(cache, filename, create));
    if baseline.is_null() || filename.is_null() || !globals_initialized() {
        return baseline;
    }
    route_config(cache, &*filename, baseline)
});

// UEngine::LoadMap — select destination context early and restore it if loading fails.
define_pattern_resolver!(ConfigLoadMap, [
    "48 89 5C 24 20 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 50 FE FF FF 48 81 EC B0 02 00 00 0F 29 BC 24 90 02 00 00"
]);
CREATE_HOOK!(ConfigLoadMap, { || cli_args().unchained_profile_location.is_some() }, NONE, bool, (
    engine: *mut UEngine, world_context: *mut FWorldContext, url: *const FURL, pending: *mut UPendingNetGame, error: *mut FString
), {
    let previous = url.as_ref().and_then(|url| url.map.copy_to_string().ok())
        .and_then(|map| config_overrides::select_map(&map, None));
    let loaded = CALL_ORIGINAL!(ConfigLoadMap(engine, world_context, url, pending, error));
    if !loaded {
        if let Some(previous) = previous {
            config_overrides::restore_map(previous);
        }
    }
    loaded
});

// UTBLGameInstance::CreateGameModeForURL — both ordinary and seamless world creation.
// FURL is passed by value in C++; Windows x64 lowers it to a pointer to a caller temporary.
// The original function destroys that temporary. Borrow it only before calling the original.
define_pattern_resolver!(ConfigCreateGameMode, [
    "40 53 55 56 57 41 56 48 81 EC C0 02 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 84 24 B0 02 00 00 48 8B D9 49 8B F8"
]);
CREATE_HOOK!(ConfigCreateGameMode, { || cli_args().unchained_profile_location.is_some() }, NONE, *mut AGameModeBase, (
    game_instance: *mut UTBLGameInstance, url: *mut FURL, world: *mut UWorld
), {
    if let Some(map) = url.as_ref().and_then(|url| url.map.copy_to_string().ok()) {
        config_overrides::select_map(&map, Some(world as usize));
    }
    CALL_ORIGINAL!(ConfigCreateGameMode(game_instance, url, world))
});

// ATBLGameMode::ProcessServerTravel — called by UWorld::ServerTravel after CanServerTravel.
// ActuallyLeaveMap has already advanced MapListIndex. Preview calls never reach this hook.
define_pattern_resolver!(ConfigProcessServerTravel, [
    "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 41 56 41 57 48 83 EC 30 48 8B 35 ?? ?? ?? ?? 41 0F B6 E8 4C 8B F2 4C 8B F9"
]);
CREATE_HOOK!(ConfigProcessServerTravel, { || cli_args().unchained_profile_location.is_some() }, NONE, (), (
    game_mode: *mut ATBLGameMode, url: *const FString, absolute: bool
), {
    let pending = game_mode.as_ref().and_then(|game_mode| {
        if !game_mode.b_use_maplist || game_mode.b_first_bot_match { return None }
        let index = game_mode.map_list_index;
        let entry = game_mode.maplist.get(usize::try_from(index).ok()?)?.copy_to_string().ok()?;
        let map = url.as_ref()?.copy_to_string().ok()?;
        config_overrides::capture_rotation(&map, index, Some(&entry))
    });
    config_overrides::accept_travel(pending);
    CALL_ORIGINAL!(ConfigProcessServerTravel(game_mode, url, absolute));
});

const _: () = {
    assert!(std::mem::offset_of!(ATBLGameMode, map_list_index) == 0x880);
    assert!(std::mem::offset_of!(ATBLGameMode, maplist) == 0x848);
    assert!(std::mem::offset_of!(ATBLGameMode, b_use_maplist) == 0x858);
    assert!(std::mem::offset_of!(ATBLGameMode, b_first_bot_match) == 0x3aa);
};
