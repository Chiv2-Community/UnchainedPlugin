# Native source layout

## Contents

- [Native ownership](#native-ownership)
- [Path rules](#path-rules)
- [Declaration order](#declaration-order)
- [Plugin behavior](#plugin-behavior)

## Native ownership

Organize native declarations by their original source owner.

- `src/chivalry2/engine/`: UE **4.25-plus** headers and engine plugins.
- `src/chivalry2/tbl/`: native TBL owners.
- `src/chivalry2/tbl_loading_screen/`: the separate loading-screen module.
- `src/chivalry2/plugins/`: native game-plugin headers, including private headers.
- Identify the owner in the file's opening source comment.
- Use native source and shipping PDB evidence where available.
- Do not treat generated per-type SDK filenames as original header ownership.
- Keep native layouts, signatures, resolvers, and bindings with their owners.
- Keep unproven resolver ownership in the
  [unresolved-target quarantine](../src/resolvers/README.md); do not guess a header.

## Path rules

Mirror source categories, modules, and header directories.

- Omit `Engine/Source`, `Public`, `Private`, and `Classes`.
- Preserve `Runtime` as `runtime` and `ThirdParty` as `third_party`.
- Snake-case native header basenames.
- Keep engine plugins under `chivalry2/engine/plugins/`.
- Use `chivalry2/plugins/<plugin path>/<header>.rs` for game plugins.
- Drop repeated `Source/<same module>` scaffolding in game-plugin paths.
- Retain distinct module directories, such as `OpenAPIMerged`.

Engine header:

```text
Engine/Source/Runtime/Engine/Classes/Engine/World.h
→ src/chivalry2/engine/runtime/engine/engine/world.rs
```

Game-plugin header:

```text
TBL/Plugins/OnlineSubsystemPlayFab/Source/OpenAPIMerged/Public/OpenAPIClientApi.h
→ src/chivalry2/plugins/online_subsystem_playfab/open_api_merged/open_api_client_api.rs
```

## Declaration order

Keep callable native contracts easy to find.

- Source comment and imports.
- Resolver declarations, separated by blank lines.
- Each native struct followed by its inherent impl.
- Trait implementations, supporting functions, and tests.
- Use `mod.rs` as a module index.
- Keep static calls static; genuine UE globals remain free functions.
- Preserve C++ method spelling on native bindings and vtable entries.
- Name native C++ destructors `Destructor`; preserve deleting-thunk ABI details.
- Preserve acronyms such as `TBL` in native identifiers.
- Name project-defined method/value resolvers `Class__Name` or `Name`.
- Keep Rust-only helpers and trait methods in Rust style.
- Retain ABI/safety/source notes; omit comments that just repeat signatures.
- Put Rust adapters beside the native types they adapt.
- See [Creating game hooks](../CONTRIBUTING.md#creating-game-hooks) for binding
  and vtable examples.

## Plugin behavior

Organize plugin policy by functionality, not native source paths.

- Detours and instruction patches: `src/unchained/patches/`.
- Custom blueprint layouts and adapters: `src/unchained/blueprints/`.
- Keep queues, chat helpers, mounted-pak tracking, and override state out of native types.
- Put detours after imports, before instruction patches, helpers, state, and tests.
- See [native bindings and plugin detours](native-bindings.md) for call contracts.
