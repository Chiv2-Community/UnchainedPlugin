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

Destination names come from the travel URL, not the outgoing world. Rotation indices
come from `ATBLGameMode::GetMapNameAtIndex`; arbitrary travel without a matching
rotation selection uses only the map layer. No outgoing-world index is guessed.
Directory components are validated; paths and Windows device names are rejected.

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
- Copies are non-saving. The engine's baseline and source config remain unchanged.
  Each new map generation rebuilds copies from that baseline, not the previous map.
- `LoadMap` selects layers before loading the destination. `SetGameMode` also selects
  them before spawning GameMode, covering seamless destination setup.
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
- Failed ordinary `LoadMap` restores the previous selection. Travel timing and
  native merges still require validation on a running server.

## Binary verification

Binary Ninja MCP inspected `Chivalry2-Win64-Shipping.exe`, SHA-256
`188c99263209244b5d0d1c18003b3e572973b3e93454295fe22089bdc33a40e1`.
Addresses are evidence for that executable, not hardcoded hook addresses.
Resolvers in `src/resolvers/config_overrides/native.rs` use byte patterns.

| Function | Address | Windows x64 arguments |
| --- | --- | --- |
| `FConfigFile` copy constructor | `0x141c0ad00` | destination, source |
| `FConfigFile` destructor | `0x141c0bb50` | this |
| `FConfigFile::AddDynamicLayerToHeirarchy` | `0x141c0e360` | this, const FString* |
| `FConfigCacheIni::Find` | `0x141c14160` | this, const FString*, bool; returns FConfigFile* |
| `UObject::LoadConfig` | `0x141e21390` | this, UClass*, const TCHAR*, uint32, FProperty* |
| `UEngine::LoadMap` | `0x14355c690` | this, context, FURL*, pending game, error; returns bool |
| `UWorld::SetGameMode` | `0x1435bc390` | this, FURL*; returns bool |
| `ATBLGameMode::GetMapNameAtIndex` | `0x1417d6770` | this, hidden FString return storage, const int32*; returns storage |

Also identified `AddStaticLayersToHierarchy` at `0x141c0ee80`,
`LoadExternalIniFile` at `0x141c1e080`, and the hierarchy loader at `0x141c1e820`.
Static hierarchy injection alone is unsuitable: saved settings can mask source layers,
and travel does not automatically rebuild them.

Verified layout: `sizeof(FConfigFile)=0x140`, `NoSave=0x51`,
`SourceIniHierarchy=0x60`, `SourceConfigFile=0xd8`, `FURL::Map=0x28`.
The native copy constructor shallow-copies `SourceConfigFile`; the plugin clears
that pointer in its private copy before merging or destruction.

## Server validation checklist

1. Apply different scalar and bot-count settings to maps A and B. Travel A → B → A.
2. Travel from A to a map with no override; confirm baseline values return.
3. Repeat with ordinary and seamless travel, same-map reload, and failed travel.
4. Exercise `+`, `.`, `-`, and `!` arrays against inherited values and saved settings.
5. Check map-index/profile precedence and manual travel without a rotation index.
6. Confirm saved INIs remain unchanged and persistent objects behave as documented.
