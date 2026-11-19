# Native source layout

`src/chivalry2/engine` mirrors UE **4.25-plus** headers.
`src/chivalry2/tbl` mirrors native TBL owners; `tbl_loading_screen` is a separate native module.
`src/chivalry2/plugins` mirrors native game-plugin headers.

- Omit `Engine/Source`, `Public`, `Private`, and `Classes`.
- Preserve source categories: `Runtime` becomes `runtime`; `ThirdParty` becomes
  `third_party`. Engine plugins remain under `chivalry2/engine/plugins`.
- Preserve original module/header directories; snake-case native header basenames,
  not generated per-type SDK filenames.
- Plugin headers use `plugins/<plugin path>/<header>.rs`. Drop repeated
  `Source/<same module>` scaffolding, but retain distinct module directories.
- Identify the original native owner at the top of each file, using shipping PDB
  evidence where recovered. `mod.rs` files are indexes.
- Keep native layouts, signatures, resolvers, and bindings with their owners.
  Static native calls stay static; genuine UE globals remain free functions.
- Native game-plugin ABIs belong under `chivalry2/plugins`, including private
  headers. Plugin-owned behavior, detours, and patches belong under `src/unchained/`.
  Additional Rust adapters may live beside the native types they adapt.

Example:

```text
Engine/Source/Runtime/Engine/Classes/Engine/World.h
→ src/chivalry2/engine/runtime/engine/engine/world.rs
```
