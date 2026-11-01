use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::Mutex,
};

use crate::{
    game::{chivalry2::ATBLGameMode, engine::UWorld},
    tools::hook_globals::{cli_args, globals, globals_initialized},
    ue::{EObjectFlags, FConfigFile, FString, UClass, UObject, FURL},
};

use super::{
    enter_map, override_paths,
    travel::{capture_rotation, restore_context},
    with_state, Context,
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
    cache: *mut c_void, section: *const u16, key: *const u16, value: i32, filename: *const FString
), {});

// Keep the native cache identity and filename, not a borrowed FConfigFile pointer.
static GAME_CONFIG: Mutex<Option<(usize, String)>> = Mutex::new(None);

fn game_config() -> Option<(usize, String)> {
    GAME_CONFIG
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn index_key() -> (Vec<u16>, Vec<u16>) {
    (
        "/Script/TBL.TBLGameMode\0".encode_utf16().collect(),
        "MapListIndex\0".encode_utf16().collect(),
    )
}

fn capture_startup(map: &str) {
    let starting_map = cli_args().next_map.as_deref();
    if !with_state(|state| state.startup.can_capture(map, starting_map)) {
        return;
    }
    let Some((cache, name)) = game_config() else {
        return;
    };
    let filename = FString::from(name.as_str());
    let baseline = CALL_ORIGINAL!(ConfigCacheFind(cache as *mut c_void, &filename, false));
    if baseline.is_null() {
        return;
    }
    let (section, key) = index_key();
    let mut index = 0;
    let index = CALL_ORIGINAL!(ConfigFileGetInt(
        baseline,
        section.as_ptr(),
        key.as_ptr(),
        &mut index
    ))
    .then_some(index);
    with_state(|state| {
        if let Some(selection) = state.startup.capture(map, starting_map, index) {
            state.pending_rotation = Some(selection);
        }
    });
}

fn preserve_rotation_index(index: i32) {
    let Some((cache, name)) = game_config() else {
        return;
    };
    let filename = FString::from(name.as_str());
    let (section, key) = index_key();
    // Rotation SaveConfig has already written to the non-saving outgoing overlay.
    // Bypass Find layering for this one scalar so the next generation starts at the live slot.
    with_native_layer((), || {
        CALL_ORIGINAL!(ConfigCacheSetInt(
            cache as *mut c_void,
            section.as_ptr(),
            key.as_ptr(),
            index,
            &filename
        ))
    });
}

struct OwnedConfig(Box<FConfigFile>);

impl OwnedConfig {
    unsafe fn copy(source: *const FConfigFile) -> Self {
        let mut file = Box::new(std::mem::zeroed::<FConfigFile>());
        CALL_ORIGINAL!(ConfigFileCopy(&mut *file, source));
        // Do not mutate or destroy the baseline's source config through the shallow copy.
        file.source_config_file = std::ptr::null_mut();
        file.no_save = 1;
        Self(file)
    }

    fn ptr(&self) -> *mut FConfigFile {
        (&*self.0 as *const FConfigFile).cast_mut()
    }
}

impl Drop for OwnedConfig {
    fn drop(&mut self) {
        CALL_ORIGINAL!(ConfigFileDestroy(self.ptr()));
    }
}

#[derive(Default)]
struct OverlayCache {
    generation: u64,
    // Cache misses too: normal config reads must not repeatedly probe the filesystem.
    files: HashMap<(usize, String, usize), Option<OwnedConfig>>,
}

thread_local! {
    // UE returns borrowed section/file pointers. Per-thread copies avoid freeing a reader's
    // storage when another thread changes map context. No RefCell borrow spans a native call.
    static CACHE: RefCell<OverlayCache> = RefCell::new(OverlayCache::default());
    static NATIVE_LAYER_SCOPE: RefCell<()> = const { RefCell::new(()) };
}

/// Native merging/destruction may reenter Find. Skip overlays for nested calls on this
/// thread; a blocking mutex would deadlock, and a global try-lock would skip other readers.
fn with_native_layer<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    NATIVE_LAYER_SCOPE.with(|scope| {
        let Ok(_borrow) = scope.try_borrow_mut() else {
            return fallback;
        };
        f()
    })
}

// FConfigCacheIni::Find — intercept config lookup to return a private merged config.
define_pattern_resolver!(ConfigCacheFind, [
    "40 53 56 41 54 41 56 48 81 EC A8 01 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 84 24 90 01 00 00 8B 42 08 33 DB 85 C0 45 0F B6 E0 4C 8B F2"
]);
CREATE_HOOK!(ConfigCacheFind, { || cli_args().unchained_profile_location.is_some() }, NONE, *mut FConfigFile, (
    cache: *mut c_void, filename: *const FString, create: bool
), {
    let baseline = CALL_ORIGINAL!(ConfigCacheFind(cache, filename, create));
    if baseline.is_null() || filename.is_null() || !globals_initialized() {
        return baseline;
    }
    with_native_layer(baseline, || {
    let Ok(name) = (*filename).copy_to_string() else { return baseline };
    if super::selection::config_filename(&name).is_some_and(|name| name.eq_ignore_ascii_case("Game.ini")) {
        *GAME_CONFIG.lock().unwrap_or_else(|e| e.into_inner()) = Some((cache as usize, name.clone()));
    }
    let context = with_state(|state| state.context.clone());
    let key = (cache as usize, name.clone(), baseline as usize);

    let cached = CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.generation != context.generation {
            cache.generation = context.generation;
            let old = std::mem::take(&mut cache.files);
            drop(cache);
            drop(old);
            return None;
        }
        cache.files.get(&key).map(|file| file.as_ref().map_or(baseline, OwnedConfig::ptr))
    });
    if let Some(file) = cached { return file }

    let paths = override_paths(&context.selection, &name);
    if paths.is_empty() {
        CACHE.with(|cache| { cache.borrow_mut().files.insert(key, None); });
        return baseline;
    }
    let file = OwnedConfig::copy(baseline);
    for path in &paths {
        let filename = FString::from(path.as_str());
        CALL_ORIGINAL!(ConfigFileAddLayer(file.ptr(), &filename));
    }
    let result = file.ptr();
    CACHE.with(|cache| { cache.borrow_mut().files.insert(key, Some(file)); });
    #[cfg(feature = "config_overrides_verbose_hooks")]
    sdebug!("INI layers for {name}: {}", paths.join(" -> "));
    result
    })
});

// UObject::LoadConfig — call-only binding.
// Used only to refresh existing class defaults before destination gameplay is initialized.
// Each import reads ONE already-merged config, not a sequence of individual override INIs.
define_pattern_resolver!(LoadConfig, [
    "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 D0 FC FF FF 48 81 EC 30 04 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 10 03 00 00"
]);
CREATE_HOOK!(LoadConfig, CALLED, NONE, (), (
    object: *mut UObject, class: *mut UClass, filename: *const u16, flags: u32, property: *mut c_void
), {});

fn refresh_defaults(categories: &HashSet<String>) {
    if categories.is_empty() || !globals_initialized() {
        return;
    }
    // Release the object-array lock before invoking engine code, which can allocate UObjects.
    let candidates = {
        let objects = globals().guobject_array();
        let objects = objects.objects();
        objects
            .iter()
            .flatten()
            .filter(|object| {
                object
                    .object_flags
                    .contains(EObjectFlags::RF_ClassDefaultObject)
                    && !object.object_flags.intersects(
                        EObjectFlags::RF_BeginDestroyed | EObjectFlags::RF_FinishDestroyed,
                    )
            })
            .map(|object| object as *const _ as *mut UObject)
            .collect::<Vec<_>>()
    };
    for object in candidates {
        unsafe {
            let base = &(*object).uobject_base_utility.uobject_base;
            if !globals().guobject_array_unchecked().is_live(base) {
                continue;
            }
            let Some(class) = base.class_private.as_ref() else {
                continue;
            };
            if class.class_flags & 0x4 == 0 {
                continue;
            }
            if categories.contains(&class.class_config_name.to_string().to_ascii_lowercase()) {
                CALL_ORIGINAL!(LoadConfig(
                    object,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut()
                ));
            }
        }
    }
}

fn select_map(map: &str, world: Option<usize>) -> Option<Context> {
    capture_startup(map);
    let (previous, categories) = enter_map(map, world)?;
    refresh_defaults(&categories);
    Some(previous)
}

// UEngine::LoadMap — select destination context early and restore it if loading fails.
define_pattern_resolver!(ConfigLoadMap, [
    "48 89 5C 24 20 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 50 FE FF FF 48 81 EC B0 02 00 00 0F 29 BC 24 90 02 00 00"
]);
CREATE_HOOK!(ConfigLoadMap, { || cli_args().unchained_profile_location.is_some() }, NONE, bool, (
    engine: *mut c_void, world_context: *mut c_void, url: *const FURL, pending: *mut c_void, error: *mut FString
), {
    let previous = url.as_ref().and_then(|url| url.map.copy_to_string().ok())
        .and_then(|map| select_map(&map, None));
    let loaded = CALL_ORIGINAL!(ConfigLoadMap(engine, world_context, url, pending, error));
    if !loaded {
        if let Some(previous) = previous {
            let categories = with_state(|state| {
                restore_context(&mut state.context, &mut state.pending_rotation, previous);
                let mut categories = std::mem::take(&mut state.categories);
                state.categories = super::categories_for(&state.context.selection);
                categories.extend(state.categories.iter().cloned());
                categories
            });
            refresh_defaults(&categories);
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
CREATE_HOOK!(ConfigCreateGameMode, { || cli_args().unchained_profile_location.is_some() }, NONE, *mut c_void, (
    game_instance: *mut c_void, url: *mut FURL, world: *mut UWorld
), {
    if let Some(map) = url.as_ref().and_then(|url| url.map.copy_to_string().ok()) {
        select_map(&map, Some(world as usize));
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
        capture_rotation(&map, index, Some(&entry), preserve_rotation_index)
    });
    // Every accepted travel replaces the previous snapshot, including manual/unmatched travel.
    with_state(|state| {
        state.startup.finish();
        state.pending_rotation = pending;
    });
    CALL_ORIGINAL!(ConfigProcessServerTravel(game_mode, url, absolute));
});

// Verified in the current Steam executable, not the older reference binary.
const _: () = {
    assert!(std::mem::offset_of!(ATBLGameMode, map_list_index) == 0x880);
    assert!(std::mem::offset_of!(ATBLGameMode, maplist) == 0x848);
    assert!(std::mem::offset_of!(ATBLGameMode, b_use_maplist) == 0x858);
    assert!(std::mem::offset_of!(ATBLGameMode, b_first_bot_match) == 0x3aa);
};

#[cfg(test)]
mod tests {
    use super::with_native_layer;

    #[test]
    fn nested_native_calls_use_baseline_and_release_scope() {
        assert_eq!(
            with_native_layer("baseline", || {
                with_native_layer("nested baseline", || panic!("nested overlay must not run"))
            }),
            "nested baseline"
        );
        assert_eq!(with_native_layer("baseline", || "overlay"), "overlay");
    }

    #[test]
    fn native_scope_is_released_on_unwind() {
        let result = std::panic::catch_unwind(|| {
            with_native_layer((), || panic!("test unwind"));
        });
        assert!(result.is_err());
        assert!(with_native_layer(false, || true));
    }
}
