# Map-specific INI layers

Apply different INI settings to each map or rotation slot without changing the
server's baseline settings.

## Setup

1. Set `--unchained-profile-location` to your server profile directory.
2. Put baseline settings in the server's normal INI files. Give every property you
   override an explicit baseline value so it can be restored on later maps.
3. Create override files under that profile directory:

   ```text
   <profile>/ini-overrides/maps/<destination-map>/<Config>.ini
   <profile>/ini-overrides/profiles/<profile-name>/<Config>.ini
   <profile>/ini-overrides/indices/<rotation-index>/<Config>.ini
   ```

Use the destination map's short name, such as `FFA_Courtyard`, for its map folder.
Use the exact config category filename and the game's section/property names.
For example, `maps/FFA_Courtyard/Game.ini` changes `Game.ini` settings, not
`Engine.ini` settings. Missing files are skipped.

For example, to change bot counts on Courtyard, create
`<profile>/ini-overrides/maps/FFA_Courtyard/Game.ini`:

```ini
[/Script/TBL.TBLGameMode]
BotBackfillLowBots=8
BotBackfillHighBots=10
```

Keep explicit values for both properties in the baseline `Game.ini`, even on maps
where no override is needed.

## Rotation profiles and indices

Map folders work without a profile mapping. To select profile folders by rotation
slot, create `<profile>/unchained.yaml`:

```yaml
match-rotation:
  - profile: quiet
  - profile: busy
```

Indices start at zero. Slot `0` selects `profiles/quiet/` and `indices/0/`;
slot `1` selects `profiles/busy/` and `indices/1/`.

**This file does not configure the game's map rotation.** Configure the game's
`MapList` separately and keep the YAML entries in the same order. Reordering the
array changes which profile belongs to each slot. The same map may occur in
multiple slots with different profiles.

Each entry requires a profile name, unique ignoring ASCII case. Use letters,
numbers, underscores, hyphens, and dots; no spaces, paths, trailing dots, or
Windows reserved device names. Names are limited to 128 UTF-8 bytes.

Entries may also contain `name`, `description`, `map`, and `mod-actors`. These are
display metadata only: they do not select maps, change travel, or spawn actors.
An index beyond the YAML array still selects its `indices/<index>/` directory,
but no profile directory.

### When a slot is selected

- On startup, set both the initial map (`--next-map-name`) and an explicit,
  nonnegative baseline `MapListIndex` to apply profile/index settings. Without
  that index, only map settings apply.
- During rotation, the game's current `MapListIndex` selects the slot only when
  rotation is enabled and `MapList[index]` matches the destination. Invalid or
  mismatched slots apply only map settings.
- Manual travel follows the same rule. It does not search the rotation for a
  matching map. Reloading a map can therefore select its current slot, while
  travel to a different map may apply only map settings.
- Map matching ignores ASCII case and compares short names. When both map names
  include package paths, those paths must also match. YAML `map` metadata is not
  used to establish a match.

For example, put this in the server's baseline `Game.ini`:

```ini
[/Script/TBL.TBLGameMode]
MapListIndex=0
MapList=FFA_Courtyard
MapList=FFA_Wardenglade
```

Add `--next-map-name FFA_Courtyard` to your existing server launch arguments.
With the YAML above, startup selects `profiles/quiet/` and `indices/0/`.

## Precedence

Settings apply in this order, lowest priority first:

1. Baseline game settings, including saved settings and startup overrides.
2. Matching map files.
3. Matching profile files, when a rotation slot is selected.
4. The exact rotation-index file, when a rotation slot is selected.

All three override levels can be used together. For scalar properties, the last
matching value wins.

### Wildcard folders

Use `~` to match zero or more characters anywhere in a map or profile folder
name. Multiple wildcards are allowed in one folder name:

```text
maps/FFA_~/             # starts with FFA_
maps/~Courtyard/        # ends with Courtyard
maps/FFA~yard/          # starts with FFA and ends with yard
maps/~FFA~Court~yard~/   # contains FFA, Court, and yard in that order
```

All matching folders apply. Patterns with fewer literal characters apply first;
patterns with more literal characters apply later. Exact folders always apply
last. For example, these folders apply to `FFA_Courtyard` in this order:

```text
maps/~/                 # every map
maps/F~/                # starts with F
maps/FFA_~/             # starts with FFA_
maps/FFA_Courtyard/     # exact match
```

Profile folders support the same rule: `profiles/competitive~/` matches a
selected profile named `competitive-night`. YAML profile names must be concrete
names, not wildcard patterns.

Matching ignores ASCII case. Patterns with equal numbers of literal characters
apply in alphabetical order, ignoring ASCII case. Adjacent wildcards are allowed
and behave like a single wildcard. `*` and `?` are not supported. Index folders
are exact-only.
All matching map files apply before any profile files, then the index file.

## Arrays

Override layers support `+` (unique add), `.` (append, including duplicates),
`-` (remove one matching value), and `!` (clear). For replacement, use
`!Key=ClearArray` followed by the new entries; plain assignment does not clear an
inherited array.

These operators apply to override layers, not the saved baseline
`Saved_*/Config/WindowsNoEditor/Game.ini`. In that saved file, represent arrays
with repeated literal keys, preserving order and duplicates:

```ini
[/Script/TBL.TBLGameMode]
MapList=FFA_Courtyard
MapList=FFA_Wardenglade
MapList=FFA_Courtyard
```

An override that leaves an array empty may not clear values already present in
the game. Do not rely on empty-array overrides to reset an existing array.

## Applying changes and limits

- Overrides take effect during destination-map setup, including ordinary and
  seamless travel. They are not a general live-settings reload.
- Restart after changing `unchained.yaml`. INI edits, including newly created
  files, are picked up on the next map load, not immediately.
- Map-specific settings are not written back to baseline INI files. Each map
  starts from the baseline, not the previous map's overrides. Rotation-index
  continuity is maintained during travel; persistence across restarts is not
  guaranteed.
- Persistent objects, settings copied elsewhere by the game, and startup-only
  systems may require game-specific reload support or a restart.
- A failed ordinary map load restores the previous layer selection, but does
  not roll back the game's rotation state.

## Migrating older profiles

- Rename `map-profiles/` to `profiles/` and `map-indices/` to `indices/`.
- Replace the old `map_profiles` YAML mapping with `match-rotation`; the old
  schema is rejected.
