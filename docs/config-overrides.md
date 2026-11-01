# Map-specific INI layers

Set `--unchained-profile-location` as before. Files use this layout:

```text
<profile>/ini-overrides/maps/<destination-map>/<Config>.ini
<profile>/ini-overrides/profiles/<profile-name>/<Config>.ini
<profile>/ini-overrides/indices/<rotation-index>/<Config>.ini
```

Precedence, lowest first:

1. The game's loaded config, including saved/generated settings and startup overrides.
2. The destination map's file.
3. The destination rotation profile's file.
4. The destination rotation index's file.

All three added layers are always available; no build feature is needed.
Zero-based rotation indices select entries in `ini-overrides/unchained.yaml`:

```yaml
match-rotation:
  - name: "Opening match"
    description: "First rotation entry"
    profile: quiet
    map: TDM_QuarryKP
    mod-actors: []
  - name: "Second match"
    description: "Second rotation entry"
    profile: busy
    map: TDM_QuarryKP
    mod-actors: ["ExampleActor"]
```

Index `0` selects `profiles/quiet/` followed by `indices/0/`; index `1` selects
`profiles/busy/` followed by `indices/1/`. Profile names are required, safe directory
names and must be unique (case-insensitive). Reordering entries changes index mappings.
Out-of-range indices still apply their index layer, but have no profile layer.
`name`, `description`, `map`, and `mod-actors` are optional display metadata only:
they do not choose the destination map, alter travel, or spawn actors.
Replace the old `map_profiles` mapping with this array; the old schema is rejected.

Use the exact config category filename and real UE section/property names.
Move existing `map-profiles/` and `map-indices/` folders to `profiles/` and `indices/`.
For example, `maps/<map>/Game.ini` overrides `Game.ini`, not `Engine.ini`.
Only existing files are included. The last matching layer wins for scalar keys.
These layers now apply to native and mod config consumers, not only AgMods objects.

UE parses and merges the files. Arrays support `+` (unique add), `.` (append),
`-` (remove one matching value), and `!` (clear). Use `!Key=ClearArray` before
replacement array entries; plain assignment does not clear an inherited array.

Destination names come from the travel URL, not the outgoing world. At accepted
`ATBLGameMode::ProcessServerTravel`, the plugin reads the live `MapListIndex` and
checks that `MapList[index]` matches the destination. Normal rotation advances this
field before travel. Missing, invalid, disabled, or mismatched slots select only the
map layer. Repeated maps retain their actual slot; names are never searched for an index.
Matching uses case-insensitive map leaves, plus full package comparison when both
names include a path. The YAML `map` metadata is not used for this validation.
Directory components are validated; paths and Windows device names are rejected.

For the initial configured gameplay destination (`--next-map-name`), the plugin reads
the explicit baseline `MapListIndex` before selecting layers. Frontend loads do not
consume that slot. An absent or negative index selects map-only layers; no map-name
search or YAML `map` lookup supplies a fallback. A nonnegative index without a YAML
entry still selects its exact index directory.

## Prefix wildcard folders

Use a trailing `~` for a prefix wildcard. Windows does not allow `*` in folder names.
For destination `FFA_Courtyard`, these map folders merge in this order:

```text
maps/~/                 # every map
maps/F~/                # starts with F
maps/FFA_~/             # starts with FFA_
maps/FFA_Courtyard/     # exact match, highest map-layer priority
```

The same rules apply to profile folders: `profiles/competitive~/` matches a selected
profile named `competitive-night`. YAML profile names remain concrete, unique names;
wildcards belong only in folder names. Matching is ASCII case-insensitive.
All matching files are merged, not just the most specific match. Longer prefixes
override shorter prefixes; exact names override all matching wildcard folders.
`~` matches an empty suffix too. Only one trailing wildcard is supported, not
wildcards in the middle or `?` patterns.

All map matches merge before all profile matches, then the exact index layer.
Index folders do not support wildcards.

## Runtime behavior and limits

- Each thread lazily deep-copies the loaded native `FConfigFile`, then calls UE's
  `AddDynamicLayerToHeirarchy()` for the selected files.
- Copies are non-saving. Conditional settings never enter the baseline or source config.
  Each new map generation rebuilds copies from the baseline, not the previous map.
  At accepted, validated rotation travel, only native `MapListIndex` is preserved in
  the baseline cache. Otherwise the outgoing overlay's `SaveConfig` write would be
  lost when the destination generation is rebuilt, resetting the new GameMode index.
  This is in-memory rotation continuity, not a guarantee of index persistence on disk.
- `LoadMap` selects layers before loading the destination.
  `UTBLGameInstance::CreateGameModeForURL` also selects them before spawning GameMode,
  covering ordinary and seamless destination setup. A seamless transition world
  cannot consume the final destination's pending rotation slot.
- Existing config class defaults are reimported once from the merged cache before
  destination GameMode creation. This is not a general live-settings reload:
  persistent instances, settings copied into other fields, and startup-only engine
  systems may require explicit game support or restart.
- Give every overridden property an explicit value in the baseline config. UE's
  `LoadConfig` does not reset a previously imported scalar when its key is absent
  from the new config; such class-default values can otherwise survive travel.
- Files and missing-file results are cached until the next map generation.
  Profile YAML is read once per process. Restart after editing profile mappings.
- Copies are invalidated on map changes, not on arbitrary in-process writes to
  the underlying config. Do not treat them as a general writable config cache.
- Failed ordinary `LoadMap` restores the previous selection, not native rotation state.
  Diagnostic equivalents passed startup slot 1 and all six natural slots plus wrap
  (42 value/index checks); this production implementation has not been live-tested.
  Empty arrays can retain prior class-default contents when UE skips import from an
  empty config object. Explicit baseline scalars do not resolve that array caveat.

## Binary verification

There are four active detours: config lookup (`FConfigCacheIni::Find`), ordinary
travel (`UEngine::LoadMap`), destination GameMode creation
(`UTBLGameInstance::CreateGameModeForURL`), and accepted server travel
(`ATBLGameMode::ProcessServerTravel`). The last captures the existing rotation field
while the outgoing GameMode is still alive. YAML parsing and wildcard matching need
no additional hooks.

Six other `CREATE_HOOK!` declarations are disabled, call-only bindings: the config
copy constructor, destructor, dynamic-layer API, `UObject::LoadConfig`,
`FConfigFile::GetInt`, and `FConfigCacheIni::SetInt`. These
reuse the repository's resolver/trampoline plumbing without intercepting those calls.
Shared layouts are `ue::FConfigFile` and `ue::FURL`; filenames use the existing
`ue::FString` and its engine allocator.

Native calls may reenter config lookup. A thread-local scoped borrow skips overlays
for those nested calls and releases automatically, including on unwind. A blocking
mutex would deadlock on same-thread reentry. This scope is separate from the cache:
no cache borrow spans native calls, and per-thread caches preserve reader storage
when another thread selects a new map.

Binary Ninja MCP verified the current Steam `Chivalry2-Win64-Shipping.exe`, SHA-256
`d82b5eaa122aaee15366499bbce34e325741fecd06d6b7b5490fc650b5afb48b`.
Addresses are evidence for that executable, not hardcoded hook addresses.
Resolvers in `src/resolvers/config_overrides/native.rs` use byte patterns; the original
eight and the two index-access bindings were verified against that executable.

| Function | Address | Windows x64 arguments |
| --- | --- | --- |
| `FConfigFile` copy constructor | `0x141d8e9d0` | destination, source |
| `FConfigFile` destructor | `0x141d8f820` | this |
| `FConfigFile::AddDynamicLayerToHeirarchy` | `0x141d92030` | this, const FString* |
| `FConfigCacheIni::Find` | `0x141d97f00` | this, const FString*, bool; returns FConfigFile* |
| `UObject::LoadConfig` | `0x141fa5070` | this, UClass*, const TCHAR*, uint32, FProperty* |
| `UEngine::LoadMap` | `0x1436e0ab0` | this, context, FURL*, pending game, error; returns bool |
| `UTBLGameInstance::CreateGameModeForURL` | `0x14192a390` | this, by-value FURL temporary pointer, UWorld*; returns AGameModeBase* |
| `ATBLGameMode::ProcessServerTravel` | `0x141950520` | this, const FString*, bool; returns void |
| `FConfigFile::GetInt` | `0x141d9c750` | this, const TCHAR*, const TCHAR*, int32*; returns bool |
| `FConfigCacheIni::SetInt` | `0x141da8010` | this, const TCHAR*, const TCHAR*, int32, const FString* |

Evidence for the travel boundaries:

- `ActuallyLeaveMap` (`0x14191f820`) passes `this+0x880` to `GetNextMapName`
  (`0x141934680`) at `0x14191f8bc`, then saves config and calls `ServerTravel`.
- `AwardVictory` (`0x1419245c0`) instead passes a temporary index for preview.
  `GetNextMapName` is not a travel-commit notification and is no longer intercepted.
- `UWorld::ServerTravel` (`0x143740490`) calls `CanServerTravel` before the virtual
  `ProcessServerTravel` dispatch. Rejected requests do not reach the capture hook.
- Rotation `SaveConfig` precedes this dispatch; destination GameMode creation follows
  it. The existing accepted-travel hook therefore preserves the validated index after
  saving and before rebuilding layers. No `SaveConfig`, `ActuallyLeaveMap`,
  `GetNextMapName`, or `PostLoadMap` instrumentation hooks are needed.
- Ordinary `SetGameMode` (`0x1437406b0`) and seamless handler `Tick`
  (`0x143742ba0`) both dispatch GameInstance vtable slot `0x330`. Seamless travel
  inlines the setup and bypasses `SetGameMode`, so that method is no longer hooked.
- The TBL override at `0x14192a390` calls the base GameInstance implementation
  (`0x1432d72a0`) before processing `MOD=` options. Its by-value `FURL` temporary
  is destroyed by the original function; the hook only borrows it before that call.

The initial reference executable was an older build (SHA-256
`188c99263209244b5d0d1c18003b3e572973b3e93454295fe22089bdc33a40e1`).
Its `MapListIndex` offset was `0x850`; the current build's `0x880` agrees with the
existing Rust layout. Offsets and absolute addresses must not be transferred between builds.

Verified layout: `sizeof(FConfigFile)=0x140`, `NoSave=0x51`,
`SourceIniHierarchy=0x60`, `SourceConfigFile=0xd8`, `FURL::Map=0x28`.
Current `ATBLGameMode` fields: `MapList=0x848`, `bUseMapList=0x858`,
`MapListIndex=0x880`, `bFirstBotMatch=0x3aa`.
The native copy constructor shallow-copies `SourceConfigFile`; the plugin clears
that pointer in its private copy before merging or destruction.

## Server validation checklist

1. Apply different scalar and bot-count settings to maps A and B. Travel A → B → A.
2. Travel from A to a map with no override; confirm baseline values return.
3. Repeat with ordinary and seamless travel, same-map reload, and failed travel.
4. Exercise `+`, `.`, `-`, and `!` arrays against inherited values and saved settings.
5. Check map-index/profile precedence, repeated-map slots, and manual travel whose
   destination does not match the live rotation slot. Previewing the next map must
   not affect selection; a rejected travel request must not change context.
6. Confirm conditional values never reach saved INIs; native index writes may persist.
   Check persistent objects against the documented limits.
