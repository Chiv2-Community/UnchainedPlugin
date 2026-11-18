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

- **KNOWN ISSUE — argument decoding crashes.** The initial implementation tried to read
  `InString` by copying the `FFrame` and calling `FFrame::arg()`. `arg()` drives the engine's
  `FFrame::Step`/`StepExplicitProperty`, which for a **bytecode** frame re-evaluates the script
  expression for that argument against the live VM. Called from a pre-hook (even on a copied
  frame) this caused an access violation in unrelated UMG widget rebuilding
  (`UWidget::TakeWidget_Private` / `UOverlay::RebuildWidget`) at the frontend map. It is NOT safe
  to decode exec-wrapper args this way; this needs a proper, safe reading method and is
  **unresolved**. The detours are currently restored to the decode attempt so the issue can be
  attacked, not papered over.
- Because shipping `PrintString` is dummied, there is **no** log/console output to capture via
  `FOutputDevice`/`ConsoleOutputDevice` — reading the exec wrapper's args is the only place the
  string exists, which is exactly why this was built. Reports that the string is already captured
  via the console device are incorrect.
- The `force_*` identity features are unrelated to the crash and remain safe (they only touch the
  bool out-pointer after the original runs).


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
