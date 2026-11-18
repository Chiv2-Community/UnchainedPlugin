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

## Open / next steps
- Write any of the above "useful functions" as real hooks if wanted (observe or forward).
- Wire `resolve_hook_target` so debug builds auto-hook the inner function instead of the thunk.
- Optional: earlier attach (UGameEngineTick first-tick / PreInit resolver) to catch pre-map prints.
- Runtime-verify the logging + filter end to end (`blueprint.log`, suppression).
- Nothing is pushed; branch is local.
