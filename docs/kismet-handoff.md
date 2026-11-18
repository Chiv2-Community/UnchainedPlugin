# Kismet / Blueprint-hooking — handoff

State of the Kismet native-hook work on branch `feature/kismet-diagnostics-hook`. Companion docs:
[blueprint-vm-arg-reading.md](blueprint-vm-arg-reading.md) (why/how the exec-thunk stepping works),
[kismet-exec-hooks.md](kismet-exec-hooks.md) (the macro surface + how to add a hook/class).

## What exists now

### 1. Exec-thunk hooks (Blueprint VM native functions)
Shipping folds `UKismetSystemLibrary::PrintString/PrintWarning/PrintText` C++ bodies into a shared
no-op, so we hook the **exec thunks** (`execPrintString`, …) and read the args off the `FFrame`.

- Macros in `src/macros/kismet_exec.rs`:
  - `kismet_native_class!(Class => path "/Script/...")` — reflection-backed resolver (preferred, general).
  - `kismet_native_class!(Class => globals.method)` — patternsleuth RegisterNatives table (KSL only).
  - `kismet_exec_observe!(Class::Native => detour(args…) { body })` — steps the **real** frame once
    per declared arg in order, `P_FINISH`, runs body, does **not** call the original (for dummied
    callees). `bool`→4-byte read; `FString` self-frees; `FText`→`OwnedText` (released on drop).
  - `kismet_exec_forward!(Class::Native => detour { body })` — calls the original then runs body with
    `context`/`result` (for non-dummied callees, e.g. the server-identity `force_*` overrides).
- Hooks live in `src/resolvers/messages.rs` mod `kismet_natives` (print family + identity family).
- Each submits a `resolvers::LateHookRegistration`; `postinit_rustlib` drains them **on a thread that
  waits for `ENGINE_READY`** (commit `e8c5cd1`) — see Timing below.
- The crash history (copy-frame / wrong-arg) and the fix: commit `356f01c` + blueprint-vm-arg-reading.md.

### 2. Reflection resolver (general — any class by path)
`src/ue/object.rs` (commit `315d140`):
- `find_function(class, name)` — walks `UStruct::children` (+ super chain), matches `FName::to_string()`.
- `find_native(class_path, name) -> Option<usize>` — `get_uobject_from_path` → `UClass` →
  `find_function` → read `UFunction::func` (the exec thunk address).
- `cdo_of_path(path)` — the Class Default Object (`UClass::class_default_object`).

### 3. Inner-callee extractor / dummy detection
`src/ue/exec_thunk.rs` (commit `eced1e6`):
- `inner_native_of(thunk)` — scans the thunk for the first in-module `CALL` after the last
  `FFrame::Step`/`StepExplicitProperty` = the inner C++ function the thunk calls.
- `shared_noop_stub()` — derived at runtime from `PrintString`'s inner callee.
- `is_dummied(thunk)` / `resolve_hook_target(class, name)` → `Inner(addr)` (hook directly, debug
  builds) or `Exec(thunk)` (hook the thunk + step, shipping). Utility; not yet wired into attach.

### 4. Blueprint print logging + spam filter (commit `04c8b9a`)
- All three print observers funnel through `log_blueprint_print(kind, message)` in `kismet_natives`.
- Logs to the **`blueprint_log`** log4rs target → console **and** `blueprint.log` (new appenders +
  `__BLUEPRINT_LOG_PATH__` placeholder in `src/logging/log4rs/`), mirroring the chat-log setup.
  Console: `HH:MM:SS | BP PrintString  - <msg>`; file: `<iso-ts> | BP PrintString  - <msg>`.
- **Spam filter:** `blueprint_filter.txt` in the log dir (`%LOCALAPPDATA%\Chivalry 2\Saved\Logs\`),
  one **exact full message** per line (`#` comments / blank lines ignored), loaded once at first
  print. Matching messages are suppressed. Edit + restart to change.
- `logging::log4rs::plugin_log_dir()` exposes the log dir for sidecar files.
- Feature gate: `blueprint_printstring_log` (pulled in by the `kismet_diagnostics` dev default).

### 5. `CREATE_HOOK_KISMET!` — reflection-resolved, late-attached native hooks (commit `0797bde`)
For hooking a *real* native (normal `CREATE_HOOK!` ABI) but resolving it by **reflection** instead of
a byte signature: `CREATE_HOOK_KISMET!(Class, "Native", Name, …)` (`src/macros/kismet_hook.rs`).
- Resolves `KismetNativeClass::resolve(native)` → exec thunk → `exec_thunk::inner_native_of` → the
  inner C++ fn, then attaches **late** (`LateHookRegistration`, after `ENGINE_READY`).
- Full `CREATE_HOOK!` parity (void/`ACTIVE` defaults, `{cond}`, Status/HookType); `o_<Name>` /
  `CALL_ORIGINAL!` identical. It is a **thin surface**: the detour (`@emit_core`) and initialize/enable
  (`@finish_attach`) live once in `__create_hook_impl` (`hook_macros.rs`), shared with the default
  `CREATE_HOOK!` arm — no duplication. Kept a separate macro (not a `CREATE_HOOK!` flag) to match the
  branch's "separate macro per concern" convention (`bind_resolver_func`).
- Example: `GetTBLGameMode` (`etc_hooks.rs`) switched off its byte signatures to
  `CREATE_HOOK_KISMET!(UTBLSystemLibrary, "GetTBLGameMode", GetTBLGameMode, INACTIVE, *mut ATBLGameMode, (object: *mut UObject), {…})`.
- **When to use:** only reflected (`UFUNCTION`) functions, inner callee not inlined, late attach OK.
  Otherwise use plain `CREATE_HOOK!` (signature/offset, preinit); for *dummied* functions use
  `kismet_exec_observe!/forward!`.
- **Caveat (GetTBLGameMode):** small getters may be inlined into the thunk → `inner_native_of` finds
  no distinct inner call and the hook logs `☐`; if so, revert it to the signature.

## Timing (important)
Reflection (`find_native` → `StaticFindObject`/`FindFunctionByName`) queries the **live UObject
system**, which is **not** populated at `postinit_rustlib`. Attaching there returns null for every
lookup. Fix: the `LateHookRegistration` drain runs on a thread that first waits for `ENGINE_READY`
(set from `OnPreLoadMap`, `unchained_integration.rs`). The patternsleuth table (`globals.method`)
is built by static binary analysis at scan time, so it has no such dependency — that is why it
worked earlier. There is **no** `FEngineLoop::PreInit` hook; if you want to attach earlier than the
first map load, hook the first `UGameEngineTick` (already resolved) or add a PreInit resolver.

## Build / validate
- Build from WSL via Windows Cargo: `cmd.exe /C "cd /d U:\Unchained\2026\UnchainedPlugin && cargo build"`
  (close the running game first, or the DLL overwrite fails with access-denied).
- Default features include `kismet_diagnostics`. Clean build has only pre-existing `motd` /
  `EChatType` / `notify` warnings.
- Runtime: hooks log `☑ UKismetSystemLibrary::PrintString … (Kismet native)` after the first
  map/level load (ENGINE_READY), not at startup.

## Maybe-useful functions for modding
The reflection resolver makes any surviving `UFunction` reachable via
`find_native("/Script/<Package>.<Class>", "<Func>")`. Caveat: anything wrapped in
`#if !UE_BUILD_SHIPPING` was *compiled out* and cannot be recovered — only functions/sinks that
still exist are hookable.

- **`UEngine::AddOnScreenDebugMessage`** (on `GEngine`) — on-screen debug text; intact in shipping.
- **`FOutputDeviceRedirector::Serialize` (`GLog`) / `FMsg::Logf`** — central log sink; hook once to
  capture all compiled-in `UE_LOG` output.
- **`UKismetSystemLibrary::ExecuteConsoleCommand` / `UEngine::Exec`** — intercept or drive console
  commands (console is otherwise locked down in shipping).
- **`UObject::CallFunctionByNameWithArguments`** — invoke Blueprint functions by name.
- **`FDebug::EnsureFailed` / assert handlers** — surface ensures/checks.
- **`UObjectBaseUtility::GetPathName` / `FName::ToString`** (already resolved) — identify objects.
- **`UGameplayStatics::*`** (spawn actor, get player controller/pawn, etc.) — resolve by path.
- Print family (done): `PrintString` / `PrintWarning` / `PrintText`.

## Post-merge migration — `refactor/unchained-macros`
That branch moves the hook macro into the `unchained-macros` crate, switches the detour to
`retour::lifetime_detour!` + a `OnceLock`-stored `Detour_<Name>`, and adds `bind_resolver_func` /
`bind_resolver_value` attributes (binding natives to **signature** resolvers for direct calls). None
of our kismet additions exist there, so when it lands:
- **Hook macro moved** `src/macros/hook_macros.rs` → `unchained-macros/src/hooks.rs`. Re-apply the
  shared `@emit_core`/`@finish_attach`/`@kismet` impl against the new detour: the tail becomes
  `Detour_<Name>::new(target, detour_fkt)?` → `o_<Name>.set(..)` → `o_<Name>.get().expect(..).enable()`
  (not `static_detour!`/`.initialize`/`.enable`). `CREATE_HOOK_KISMET!` stays a thin surface over it.
- **Port the kismet macros** (`kismet_exec_observe!/forward!`, `kismet_native_class!`,
  `CREATE_HOOK_KISMET!`) into `unchained-macros`, converting old `static_detour!` +
  `.initialize/.enable/.call` to the branch's `OnceLock` + `o_<Name>.get().expect(..).call(..)` form
  (as the branch's `CALL_ORIGINAL!` now does).
- **Add `LateHookRegistration` + the `ENGINE_READY` drain** to the branch (`resolvers/mod.rs` +
  `postinit_rustlib` don't have them).
- **Relocate UE types** into the branch's modular layout: `FFrame` (a `ue` kismet module),
  `exec_thunk`, and `find_function/find_native/inner_native_of/cdo_of_path` into `src/ue/object.rs` /
  `core_uobject`; reuse the branch's `static_find_object` binding instead of `get_uobject_from_path`.
- **GetTBLGameMode:** the branch binds it via `bind_resolver_func` on its **signature** (deterministic,
  early). Recommended: keep that and **do not** carry the `CREATE_HOOK_KISMET!` conversion across —
  reflection here is inlining-risky. Reserve `CREATE_HOOK_KISMET!` for reflected natives with no
  usable signature.
- **Blueprint logging + spam filter + `plugin_log_dir()`** are additive; port as-is (relocate moved types).

## Future: runtime-resolver abstraction (Option 3, post-merge)
Long-term, make resolution **data** instead of macro arms: a `RuntimeResolver` enum/trait
`resolve(&offsets) -> Result<usize>` + a `needs_engine` flag, consumed by one `attach`, with a single
registration carrying resolver + timing (preinit vs late follows from `needs_engine`). Why it beats
the `CREATE_HOOK_KISMET!` surface: new resolver kinds (signature, reflection, …) stop touching the
hook macro; it composes with the branch's `bind_resolver_*` model; and the preinit/late split becomes
automatic. Deferred until after `refactor/unchained-macros` merges.

## Open / next steps
- Write any of the above "useful functions" as real hooks if wanted (observe or forward).
- Wire `resolve_hook_target` so debug builds auto-hook the inner function instead of the thunk.
- Optional: earlier attach (UGameEngineTick first-tick / PreInit resolver) to catch pre-map prints.
- Runtime-verify the logging + filter end to end (`blueprint.log`, suppression).
- Nothing is pushed; branch is local.
