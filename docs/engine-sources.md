# Native source layout

## Contents

- [Native ownership](#native-ownership)
  - [Source links](#source-links)
- [Path rules](#path-rules)
- [Declaration order](#declaration-order)
- [Plugin behavior](#plugin-behavior)

## Native ownership

Organize native declarations by their original source owner.

- `src/Chivalry2/Engine/`: UE **4.25-plus** headers and engine plugins.
- `src/Chivalry2/TBL/`: native TBL owners.
- `src/Chivalry2/TBLLoadingScreen/`: the separate loading-screen module.
- `src/Chivalry2/Plugins/`: native game-plugin headers, including private headers.
- Identify the owner in the file's opening source comment.
- Use native source and shipping PDB evidence where available.
- Do not treat generated per-type SDK filenames as original header ownership.
- Keep native layouts, signatures, resolvers, and bindings with their owners.
- Establish resolver ownership before choosing a native path; do not guess a header.

### Source links

Binding files use one explicit source link:

```rust
//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLGameInstance.h>.
```

Use the corresponding UE header URL for engine bindings. Keep original ownership
paths in a separate `Native owner:` note when SDK paths differ. Do not invent
links for declarations absent from either source tree.

The Neovim native hover reads only `Native source:`. It remaps the repository
root using Cargo's `[env]` settings in `.cargo/config.toml`:

- `ARGON_SDK_SOURCE_LOCATION`: ArgonSDK `blob/main` by default.
- `UE_SOURCE_LOCATION`: UnrealEngine `blob/4.25-plus` by default.

Both accept HTTP(S) roots or absolute checkout directories. The checkout root
must contain `Source/` for ArgonSDK or `Engine/` for UE. Checked-in source URLs
remain canonical; these settings remap hover links, not ordinary Rustdoc URLs.

For machine-local overrides, copy `.cargo/config.local.toml.example` to
`.cargo/config.local.toml` and edit the paths. The override file is ignored by
Git, and the hover reads it automatically. Cargo itself does not auto-load that
filename; pass `--config .cargo/config.local.toml` when using it with Cargo.

Exported environment variables override configured values unless `force = true`.
The hover asks Cargo to merge configuration using the project's pinned nightly;
it does not parse TOML independently. Local links open in Neovim; HTTP(S) links
open in the browser. Reopen the hover after changing a root.

With a local root, both binding and detour hovers append the native implementation
as C++. A host-only Cargo example uses Tree-sitter C++ dev dependencies; these
are not linked into the plugin. The helper searches the linked header and C++
files in its owning module without compiling engine code or fetching sources.
It requires a unique definition; ambiguous overloads and declarations alone are
not displayed.

The first lookup builds the helper asynchronously. While loading, or when no
implementation can be displayed, the bottom note is:

> Configure local source paths in .cargo/config.local.toml to view the implementation inline

The implementation's source link and `gd`/`gD` open its local file. To test the
parser independently:

```sh
cargo test --locked -p xtask --example native-implementation
```

## Path rules

Mirror source categories, modules, and header directories.

- Omit `Source`, `Public`, `Private`, and `Classes`.
- Preserve native casing in module names, directories, and header basenames,
  including `Runtime`, `ThirdParty`, `CoreUObject`, and `TBL`.
- Use `Chivalry2` for the Rust root. For example:
  `Chivalry2::Engine::Runtime::Core::Windows::WindowsCriticalSection`.
- Keep engine plugins under `Chivalry2/Engine/Plugins/`.
- Use `Chivalry2/Plugins/<plugin path>/<header>.rs` for game plugins.
- Drop repeated `Source/<same module>` scaffolding in game-plugin paths.
- Retain distinct module directories, such as `OpenAPIMerged`.

Engine header:

```text
Engine/Source/Runtime/Engine/Classes/Engine/World.h
→ src/Chivalry2/Engine/Runtime/Engine/Engine/World.rs
```

Game-plugin header:

```text
TBL/Plugins/OnlineSubsystemPlayFab/Source/OpenAPIMerged/Public/OpenAPIClientApi.h
→ src/Chivalry2/Plugins/OnlineSubsystemPlayFab/OpenAPIMerged/OpenAPIClientApi.rs
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
- Name project-defined method resolvers `Class__Method`, free functions `Function`,
  and globals `Owner__Value` or `Value`.
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
