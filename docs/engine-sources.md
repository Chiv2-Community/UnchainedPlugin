# Native source layout

`src/chivalry2/engine` mirrors UE **4.25-plus** headers.
`src/chivalry2/tbl` contains native TBL declarations.
`src/chivalry2/plugins` mirrors native game-plugin headers.

- Omit `Engine/Source/Runtime`, `Public`, `Private`, and `Classes`.
- Preserve module/header namespaces; use snake-case Rust filenames.
- Plugin headers use `plugins/<plugin path>/<header>.rs`, without repeated
  `Source/<module>/Classes` scaffolding.
- `online/icmp.rs` omits the redundant `ICMP` module/header repetition.
- Link the native source at the top of each owner file. `mod.rs` files are indexes.
- Keep native layouts, signatures, resolvers, and bindings with their owners.
  Static native calls stay static; genuine UE globals remain free functions.
- Native game-plugin ABIs belong under `chivalry2/plugins`, including private
  headers. Plugin-owned behavior, detours, and patches belong under `src/unchained/`.
  Additional Rust adapters may live beside the native types they adapt.

Example:

```text
Engine/Source/Runtime/Engine/Classes/Engine/World.h
→ src/chivalry2/engine/engine/engine/world.rs
```
