use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    ffi::c_void,
};

use crate::{
    tools::hook_globals::{cli_args, globals, globals_initialized},
    ue::{EObjectFlags, FString, UClass, UObject},
};

use super::{enter_map, override_paths, selection::MapContext, with_state, Context};

// Verified against the loaded shipping executable. See docs/config-overrides.md.
// The copy constructor deep-copies containers but SHALLOW-copies SourceConfigFile.
#[repr(C, align(8))]
pub(crate) struct ConfigFile {
    pairs: [u8; 0x50],
    dirty: u8,
    no_save: u8,
    padding: [u8; 6],
    name: [u8; 8],
    hierarchy: [u8; 0x58],
    engine_dir: [u8; 0x10],
    project_dir: [u8; 0x10],
    source: *mut ConfigFile,
    tail: [u8; 0x60],
}

const _: () = {
    assert!(std::mem::size_of::<ConfigFile>() == 0x140);
    assert!(std::mem::offset_of!(ConfigFile, no_save) == 0x51);
    assert!(std::mem::offset_of!(ConfigFile, hierarchy) == 0x60);
    assert!(std::mem::offset_of!(ConfigFile, source) == 0xd8);
};

/// A borrowed FString header: native const-reference APIs must not own this Vec's buffer.
#[repr(C)]
pub(crate) struct StringRef {
    data: *const u16,
    num: i32,
    max: i32,
}

impl StringRef {
    fn new(wide: &[u16]) -> Self {
        let num = i32::try_from(wide.len()).expect("INI path exceeds FString length");
        Self {
            data: wide.as_ptr(),
            num,
            max: num,
        }
    }
}

#[repr(C)]
pub(crate) struct Url {
    protocol: FString,
    host: FString,
    port: i32,
    valid: i32,
    map: FString,
}

const _: () = assert!(std::mem::offset_of!(Url, map) == 0x28);

define_pattern_resolver!(ConfigFileCopy, [
    "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 20 33 ED C7 41 2C 80 00 00 00 48 89 29 0F 57 C0 48 89 69 08 48 8B F2"
]);
CREATE_HOOK!(ConfigFileCopy, CALLED, NONE, (), (dest: *mut ConfigFile, source: *const ConfigFile), {});

define_pattern_resolver!(ConfigFileDestroy, [
    "48 89 5C 24 08 48 89 74 24 10 57 48 83 EC 20 48 8B 05 ?? ?? ?? ?? 48 8B D9 48 63 0D ?? ?? ?? ?? 48 C1 E1 04 48 03 C8 48 3B C1 74 33"
]);
CREATE_HOOK!(ConfigFileDestroy, CALLED, NONE, (), (file: *mut ConfigFile), {});

define_pattern_resolver!(ConfigFileAddLayer, [
    "40 55 48 8B EC 48 83 EC 70 48 89 B4 24 90 00 00 00 48 8B F2 48 89 7C 24 68 48 8B F9 4C 89 74 24 60 45 33 F6 4C 89 75 B0 4C 89 75 B8"
]);
CREATE_HOOK!(ConfigFileAddLayer, CALLED, NONE, (), (file: *mut ConfigFile, filename: *const StringRef), {});

struct OwnedConfig(Box<ConfigFile>);

impl OwnedConfig {
    unsafe fn copy(source: *const ConfigFile) -> Self {
        let mut file = Box::new(std::mem::zeroed::<ConfigFile>());
        CALL_ORIGINAL!(ConfigFileCopy(&mut *file, source));
        // Do not mutate or destroy the baseline's source config through the shallow copy.
        file.source = std::ptr::null_mut();
        file.no_save = 1;
        Self(file)
    }

    fn ptr(&self) -> *mut ConfigFile {
        (&*self.0 as *const ConfigFile).cast_mut()
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
    static IN_NATIVE_LAYER: Cell<bool> = const { Cell::new(false) };
}

struct NativeLayerGuard;
impl NativeLayerGuard {
    fn enter() -> Option<Self> {
        IN_NATIVE_LAYER.with(|flag| (!flag.replace(true)).then_some(Self))
    }
}
impl Drop for NativeLayerGuard {
    fn drop(&mut self) {
        IN_NATIVE_LAYER.with(|flag| flag.set(false));
    }
}

define_pattern_resolver!(ConfigCacheFind, [
    "40 53 56 41 54 41 56 48 81 EC A8 01 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 84 24 90 01 00 00 8B 42 08 33 DB 85 C0 45 0F B6 E0 4C 8B F2"
]);
CREATE_HOOK!(ConfigCacheFind, { || cli_args().unchained_profile_location.is_some() }, NONE, *mut ConfigFile, (
    cache: *mut c_void, filename: *const FString, create: bool
), {
    let baseline = CALL_ORIGINAL!(ConfigCacheFind(cache, filename, create));
    if baseline.is_null() || filename.is_null() || IN_NATIVE_LAYER.with(Cell::get) {
        return baseline;
    }
    let Some(_guard) = NativeLayerGuard::enter() else { return baseline };
    let Ok(name) = (*filename).copy_to_string() else { return baseline };
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
        let wide: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
        CALL_ORIGINAL!(ConfigFileAddLayer(file.ptr(), &StringRef::new(&wide)));
    }
    let result = file.ptr();
    CACHE.with(|cache| { cache.borrow_mut().files.insert(key, Some(file)); });
    #[cfg(feature = "config_overrides_verbose_hooks")]
    sdebug!("INI layers for {name}: {}", paths.join(" -> "));
    result
});

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
    let (previous, categories) = enter_map(map, world)?;
    refresh_defaults(&categories);
    Some(previous)
}

define_pattern_resolver!(ConfigLoadMap, [
    "48 89 5C 24 20 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 50 FE FF FF 48 81 EC B0 02 00 00 0F 29 BC 24 90 02 00 00"
]);
CREATE_HOOK!(ConfigLoadMap, { || cli_args().unchained_profile_location.is_some() }, NONE, bool, (
    engine: *mut c_void, world_context: *mut c_void, url: *const Url, pending: *mut c_void, error: *mut FString
), {
    let previous = url.as_ref().and_then(|url| url.map.copy_to_string().ok())
        .and_then(|map| select_map(&map, None));
    let loaded = CALL_ORIGINAL!(ConfigLoadMap(engine, world_context, url, pending, error));
    if !loaded {
        if let Some(previous) = previous {
            let categories = with_state(|state| {
                let generation = state.context.generation.wrapping_add(1);
                state.context = Context { generation, ..previous };
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

define_pattern_resolver!(ConfigSetGameMode, [
    "48 89 6C 24 18 56 48 81 EC 90 00 00 00 48 8B F1 48 8B EA 48 8B 49 38 48 85 C9 75 0C 48 8B 8E F0 00 00 00"
]);
CREATE_HOOK!(ConfigSetGameMode, { || cli_args().unchained_profile_location.is_some() }, NONE, bool, (
    world: *mut c_void, url: *const Url
), {
    if let Some(map) = url.as_ref().and_then(|url| url.map.copy_to_string().ok()) {
        select_map(&map, Some(world as usize));
    }
    CALL_ORIGINAL!(ConfigSetGameMode(world, url))
});

define_pattern_resolver!(
    ConfigNextMap,
    ["40 53 57 41 56 41 57 48 83 EC 38 80 B9 AA 03 00 00 00 4D 8B F8 4C 8B F2 48 8B D9 74 4A"]
);
CREATE_HOOK!(ConfigNextMap, { || cli_args().unchained_profile_location.is_some() }, NONE, *mut FString, (
    game_mode: *mut c_void, output: *mut FString, index: *const i32
), {
    let result = CALL_ORIGINAL!(ConfigNextMap(game_mode, output, index));
    if let Some(map) = result.as_ref().and_then(|map| map.copy_to_string().ok()) {
        let selection = MapContext::new(&map, index.as_ref().copied());
        with_state(|state| state.pending_rotation = Some(selection));
    }
    result
});
