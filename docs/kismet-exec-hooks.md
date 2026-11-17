# Kismet native-exec hooks: how they work & how to add one

This covers the `kismet_native_class!` / `kismet_exec_observe!` / `kismet_exec_forward!` macros
(`src/macros/kismet_exec.rs`) used to hook Unreal **UFunction native exec thunks** such as
`execPrintString`. For *why* the detour has to step the frame the way it does, read
[blueprint-vm-arg-reading.md](blueprint-vm-arg-reading.md) first — that is the dangerous part;
this doc is about the plumbing around it.

## What we're hooking

A `BlueprintCallable` native function (e.g. `UKismetSystemLibrary::PrintString`) is dispatched by
the Blueprint VM through a generated **native exec thunk** named `exec<Name>` (`execPrintString`).
The thunk has the signature `void(UObject* Context, FFrame& Stack, void* Result)` and unpacks the
call's arguments out of the VM frame before invoking the plain C++ body. On shipping builds the C++
body of the print functions is folded to a no-op, so the thunk is the only place the arguments
exist — hence we hook the thunk.

## How the target is found — just from the name

No signature scan. It's Unreal's own reflection data:

1. At startup, each UCLASS runs its generated `RegisterNatives` call, handing the engine an array of
   `{ "PrintString", &execPrintString }` pairs — the reflected UFunction name next to its thunk
   pointer.
2. During `init_globals()`, a resolver locates `UKismetSystemLibrary`'s table and builds a
   `HashMap<String, usize>` (name → absolute thunk address), stored in `Globals`.
3. `globals().kismet_system_library_native("PrintString")` is then just a hashmap lookup returning
   the thunk's absolute address.

So "find by name" = read the game's own name→pointer table. The address is absolute (already
relocated), unlike BuildInfo's image-relative offsets.

**Consequence:** each class has its own `RegisterNatives` table, so **each class needs its own
`globals()` resolver** that finds that table at init. The macros only *wrap* an existing resolver;
they do not do the scanning.

## When they attach — late, all at once

The table lives in `Globals`, which doesn't exist during preinit. So these hooks **cannot** use the
normal `CREATE_HOOK!` path (its `HookRegistration` inventory is drained by `attach_hooks()` during
preinit). Instead:

- Each hook macro emits an `inventory::submit!(resolvers::LateHookRegistration { class, native,
  attach })`.
- `postinit_rustlib()` (after `init_globals()`) drains that inventory once:

  ```rust
  for hook in inventory::iter::<resolvers::LateHookRegistration> {
      match unsafe { (hook.attach)() } {
          Ok(())  => sinfo!("☑ {}::{} (Kismet native)", hook.class, hook.native),
          Err(e)  => serror!("☐ {}::{}: {}", hook.class, hook.native, e),
      }
  }
  ```

Only enabled hooks compile (each is behind its `#[cfg(feature = …)]`), so the drain naturally
contains exactly the active set. There is no per-module `attach_all`.

## The macros

### `kismet_native_class!(Class => globals.method)`

Binds a reflected class to its resolver. Expands to a unit struct `Class` and
`impl KismetNativeClass for Class` whose `resolve(name)` calls `globals().method(name)` and whose
`CLASS_NAME` is `stringify!(Class)`. Declare it once per class, in the same module as its hooks.

```rust
kismet_native_class!(UKismetSystemLibrary => globals.kismet_system_library_native);
```

### `kismet_exec_observe!(Class::Native => detour(args…) { body })`

For thunks whose callee is a no-op (the print family): it **consumes** the frame itself.

Generates one `static_detour!`, the attach fn, the `LateHookRegistration` submit, and the detour,
which:

1. steps each declared argument off the **real** frame, in order, via `FFrame::arg` (see below);
2. runs `P_FINISH` (`frame.finish()`);
3. runs `{ body }`;
4. returns **without** calling the original.

Argument types in the list map to steps:

| written type   | how it's stepped                                                        |
|----------------|-------------------------------------------------------------------------|
| `bool`         | reads a 4-byte VM bool, binds `name: bool`                              |
| `FString`      | owned; frees its engine buffer on drop (GMalloc)                        |
| `FText`        | wrapped in `OwnedText` (releases its shared ref on drop); read via `.to_string()` |
| anything else  | `FFrame::arg::<T>()` (e.g. `*mut UObject`, `[f32; 4]`, `f32`)            |

The list must match the thunk's real parameters **exactly, in order** — verify it against the
decompiled thunk. Prefix unused args with `_`.

### `kismet_exec_forward!(Class::Native => detour { body })`

For thunks whose callee does real work and you only want to observe/override the result (the
server-identity family). It calls the **original** (which steps + finishes the frame), then runs
`{ body }` with `context`, `stack`, and `result` in scope — e.g. `*result.cast::<bool>() = true;`.
It does **not** step the frame (the original already did).

## Recipe: add a new observe/forward hook on an existing class

1. Confirm the native is in that class's table (the resolver already covers the class).
2. Decompile the `exec<Name>` thunk and read off the parameter order/types (and whether there's a
   leading `WorldContextObject`). This is the step that prevents crashes.
3. Add one macro call next to the others, behind the right feature gate:

   ```rust
   #[cfg(feature = "my_feature")]
   kismet_exec_observe!(UKismetSystemLibrary::GetPlatformName => exec_get_platform_name(
       _world: *mut UObject, name: FString,
   ) {
       if let Ok(s) = name.copy_to_string() { sinfo!("[BP GetPlatformName] {s}"); }
   });
   ```

That's it — it auto-registers and attaches in the postinit drain. No `attach_all` edit, no lib.rs
edit.

## Recipe: add a new class

The macros can't resolve a class with no table built, so:

1. Add a resolver in the globals/patternsleuth layer that locates **that class's** `RegisterNatives`
   table and exposes `globals().<class>_native(name) -> Option<usize>` (mirror
   `kismet_system_library_native` / `resolution.kismet_system_library`).
2. `kismet_native_class!(UGameplayStatics => globals.gameplay_statics_native);`
3. Add hooks with `kismet_exec_observe!(UGameplayStatics::… => …)` as above.

## Gotchas

- **One `static_detour!` per macro invocation.** retour's macro recursion blows up if several are
  combined; each hook macro emits exactly one.
- **Order and type of the arg list is load-bearing.** A wrong order/type steps the wrong expression
  on the live VM and can crash (see blueprint-vm-arg-reading.md). Verify against the binary.
- **`observe` must not call the original; `forward` must not step.** Doing both double-consumes the
  frame.
- Resolution can fail (table not found / name absent): the attach returns `Err`, logged as `☐`, and
  the rest still attach.
