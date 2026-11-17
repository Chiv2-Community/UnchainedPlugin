# Kismet native-registry hooks

Opt-in diagnostics that hook `UKismetSystemLibrary` Blueprint VM wrappers. They
resolve targets from the **RegisterNatives** table instead of a byte signature,
so they work on shipping builds where `PrintString`'s direct C++ implementation
has been folded away.

## Features

| Feature | What it does |
|---|---|
| `kismet_log` | Log `FFrame::KismetExecutionMessage` (existing; errors/warnings). |
| `blueprint_printstring_log` | Log Blueprint `PrintString`, `PrintWarning`, and `PrintText` calls to the plugin console. |
| `kismet_diagnostics` | Umbrella = `kismet_log` + `blueprint_printstring_log`. |
| `kismet_server_identity_hooks` | Native-resolution hooks for Blueprint `IsDedicatedServer` / `IsServer` / `IsStandalone`. Results are preserved unless a force feature is enabled. |
| `force_kismet_is_dedicated_server` | Force `IsDedicatedServer` to return `true` (implies `kismet_server_identity_hooks`). |
| `force_kismet_is_server` | Force `IsServer` to return `true`. |
| `force_kismet_is_standalone` | Force `IsStandalone` to return `true`. |

The `force_*` features are deliberately separate so a single build cannot force
contradictory states (e.g. dedicated-serve and standalone at once).

## Why the native registration table

In Unreal, Blueprint-callable native functions are registered in a per-class
table. For `UKismetSystemLibrary` it maps the reflected name to the VM exec
wrapper:

```text
"PrintString" -> UKismetSystemLibrary::execPrintString
```

The `KismetSystemLibrary` resolver reads that table, so
`globals().kismet_system_library_native("PrintString")` yields the absolute
runtime address of `execPrintString`. This avoids:

- a build-specific byte signature;
- reliance on the direct `PrintString` C++ method (folded to a no-op in shipping).

## Safety notes

- **Never call `FFrame::arg` on the real `Stack`.** It advances the frame cursor
  and would corrupt the original exec handler's input. Each detour bitwise-copies
  the `FFrame` (raw-pointer state, no ownership/Drop) and decodes from the copy.
- An opt-in build. Copying a bytecode `FFrame` before the real handler can, on
  uncommon paths, evaluate a Blueprint argument expression a second time.
- `PrintText` decodes an `FText` and renders it with the existing
  `FText::ToString` helper.

## Attachment timing

These hooks **must not** register in the `inventory` drained by `attach_hooks()`:
that runs during preinit (`DllMain`), before `init_globals()` populates
`GLOBALS`, so `globals()` would panic (a non-unwinding context in `DllMain`).

They instead expose `attach_all()` and are called from `postinit_rustlib()`,
after `init_rustlib()` → `init_globals()` has run. They attach only once the
RegisterNatives table is available.

- `src/resolvers/messages.rs` → `blueprint_printstring::attach_all()`
- `src/resolvers/messages.rs` → `kismet_server_identity::attach_all()`
- `src/lib.rs` → `postinit_rustlib()`

A commented `CREATE_KISMET_NATIVE_HOOK!` macro template lives in
`src/macros/hook_macros.rs` for future cleanup; the active hooks intentionally
stay explicit because their `FFrame`-copy and `result`-out-pointer behavior is
sensitive.
