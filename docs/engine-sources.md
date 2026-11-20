# Native source layout

## Contents

- [Native ownership](#native-ownership)
  - [Source links](#source-links)
- [Path rules](#path-rules)
- [Declaration order](#declaration-order)
- [Plugin behavior](#plugin-behavior)

## Native ownership

Organize native declarations by verified source/header paths.

- `src/Chivalry2/Engine/`: UE **4.25-plus** headers and engine plugins.
- `src/Chivalry2/TBL/`: native TBL owners.
- `src/Chivalry2/TBLLoadingScreen/`: the separate loading-screen module.
- `src/Chivalry2/Plugins/`: native game-plugin headers, including private headers.
- Identify the owner in the file's opening source comment.
- Use native source and shipping PDB evidence where available.
- Link the header that defines a declaration, not one that only references it.
- SDK header paths can differ from original shipping ownership; preserve supported
  original paths as notes, not placement constraints.
- Keep native layouts, signatures, resolvers, and bindings with their owners.
- Establish resolver ownership before choosing a native path; do not guess a header.

### Source links

Binding files with available native sources use one explicit source link:

```rust
//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLGameInstance.h>.
```

Use the corresponding UE header URL for engine bindings. Keep original ownership
paths in a separate `Native owner:` note when SDK paths differ. Do not invent
links for declarations absent from either source tree; state
`Native source unavailable:` instead. Directory-index `mod.rs` files are exempt.

The Rust build-time documentation generator reads only `Native source:`. It
remaps the repository root using `[env]` settings in `.cargo/config.toml`:

- `ARGON_SDK_SOURCE_LOCATION`: ArgonSDK `blob/main` by default.
- `UE_SOURCE_LOCATION`: UnrealEngine `blob/4.25-plus` by default.

Both accept HTTP(S) roots or absolute checkout directories. The checkout root
must contain `Source/` for ArgonSDK or `Engine/` for UE. The macros emit standard
Rustdoc, including the remapped links and source excerpts. No editor-specific
substitution is used.

For machine-local overrides, copy `.cargo/config.local.toml.example` to
`.cargo/config.local.toml` and edit the paths. The override file is ignored by
Git, and the Rust documentation generator reads it automatically. No additional
Cargo arguments or editor configuration are required for native documentation.

Local values override the checked-in GitHub defaults. Non-default exported
environment values take precedence unless `force = true`. Cargo tracks changes
to configuration, Rust declarations, and local native sources. Rebuild after
changing them; editors that run Cargo build scripts refresh the same Rustdoc.
Link-opening behavior is provided by the editor, not custom project mappings.

With a local root, binding and detour docs append the native implementation as
C++. `unchained-native-docs` uses Tree-sitter C++ as a host build dependency; it
is not linked into the plugin. It searches the linked header and C++ files in
its owning module without compiling engine code or fetching native sources.
It requires a unique definition; ambiguous overloads and declarations alone are
not displayed.

Types annotated with `runtime_bindings` receive their complete C++ definition
from the linked header, including bases, members, and inline methods. Generic
structs, enums, unions, and aliases can carry documentation-only type markers.
Lookup ignores forward declarations and refuses ambiguous definitions.
Unannotated Rust helpers retain their ordinary documentation.

When local paths are not configured, the bottom note is:

> Configure local source paths in .cargo/config.local.toml to view the implementation inline

When a configured source tree has no available definition, or no source header
is linked, the note is: `No cpp sources available`.

Hover a native method/type or a detour function to view its generated Rustdoc.
Macro names themselves retain their generic macro documentation. The same
content is available through LSP and `cargo doc`. To test generation and parsing:

```sh
cargo test --locked -p unchained-native-docs
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
