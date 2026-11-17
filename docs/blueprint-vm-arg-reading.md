# Reading arguments from a Blueprint VM exec hook (PrintString & friends)

This explains how Unreal's Blueprint VM dispatches a native node like `PrintString`, why an earlier
attempt to read its argument crashed, and the pattern the detours now use. It is the companion to
the code in:

- `src/ue/kismet.rs` — `FFrame`, `FFrame::arg::<T>()`, `FFrame::finish()`
- `src/resolvers/messages.rs` — `mod blueprint_printstring` (the three detours + attach)
- `src/resolvers/messages/broadcast_display.rs` — `native_string()`, `release_text()`

## TL;DR

- `FFrame` is **not an operand stack**; it is the interpreter's *call frame* — an instruction
  pointer (`Code`) + context object + locals. It is named `Stack` by Unreal convention only.
- `FFrame::Step(Context, Result)` means **"evaluate the next argument expression in the bytecode and
  write its value to `Result`"**. It advances `Code` and can have side effects (arg expressions can
  be getters/function calls). It's a *pull* model: the callee pulls args in order, on demand.
- A native thunk (`execPrintString`) is just: step each parameter in declared order, `P_FINISH`,
  then call the C++ body. In shipping, the body is a folded **no-op**.
- **The fix:** our detour replicates the thunk — step the **real** frame once per parameter, in the
  exact order/type, `finish()`, then return **without** calling the original. This consumes the
  frame exactly once (no double side effects) and gets the real string.

## 1. Blueprints run as bytecode

A Blueprint graph compiles to a flat `uint8[]` of opcodes (`EX_*`) and inline data, stored in
`UFunction::Script`. A small interpreter walks that stream. "Calling a node" = pointing the
interpreter at some bytecode and letting it walk.

## 2. `FFrame` is the interpreter's bookmark

The fields that matter (UE 4.25 layout, confirmed against the Chivalry 2 PDB):

| offset | field    | meaning                                             |
|-------:|----------|-----------------------------------------------------|
| `0x00` | (FOutputDevice base: vtable + 2 bools, padded)     ||
| `0x10` | `Node`   | the `UFunction` being run                           |
| `0x18` | `Object` | the context / `this` the code runs against          |
| `0x20` | `Code`   | **pointer into the bytecode — the instruction pointer** |
| `0x28` | `Locals` | scratch memory for local variables                  |

`Code` is the whole game: a single pointer marching forward. Reading one opcode advances it;
evaluating one argument expression advances it past that whole sub-expression. **There is one
`Code` and everyone shares it** — moving it *is* "consuming" an argument.

(The offsets above are confirmed by `FFrame::Step`'s own disassembly: its first instructions are
`mov rax,[rcx+0x20]` / `movzx r9d,[rax]` / `inc rax` / `mov [rcx+0x20],rax` — read `Code` from
`this+0x20`, fetch one opcode byte, advance, store `Code` back.)

## 3. What a `PrintString` node compiles to

```
EX_CallMath <UFunction* UKismetSystemLibrary::PrintString>
   <bytecode for arg 0: WorldContextObject>   e.g. EX_Self / a getter
   <bytecode for arg 1: InString>             e.g. EX_StringConst "hello" / a variable / a call
   <bytecode for arg 2: bPrintToScreen>       e.g. EX_True
   <bytecode for arg 3: bPrintToLog>
   <bytecode for arg 4: TextColor>            a struct const
   <bytecode for arg 5: Duration>             EX_FloatConst 2.0
EX_EndFunctionParms                           (0x16) "args done"
```

Each argument is itself a little expression tree, **not** a precomputed value. It is evaluated when
someone steps it — and evaluating it can run code with side effects.

## 4. How dispatch reaches `execPrintString`

`PrintString` is a `FUNC_Native` UFunction; its C++ implementation pointer (`UFunction::Func`) is
`&UKismetSystemLibrary::execPrintString`. When the interpreter hits `EX_CallMath`, it calls:

```cpp
execPrintString(Context, Stack, Result);   // Stack.Code points at arg 0's bytecode
```

The arguments are **not** marshaled into a struct. The thunk must pull each one out of the live
bytecode, in order. (We resolve this address at runtime from Unreal's `RegisterNatives` table via
`globals().kismet_system_library_native("PrintString")`, never a signature.)

## 5. `Step` — the heart of it

`FFrame::Step(Context, Result)` is essentially:

```cpp
uint8 Opcode = *Code++;               // read one opcode, advance
GNatives[Opcode](Context, *this, Result);   // evaluate that expression, write result to *Result
```

So one `Step` = "consume and evaluate exactly the next one argument expression, leave its value at
`Result`, leave `Code` at the start of the following argument." The `P_GET_*` macros are just
"declare a local, then `Step` into it." A native thunk is a sequence of steps + `P_FINISH`:

```
P_FINISH   ==>   Stack->Code += (Stack->Code != 0);   // skip EX_EndFunctionParms
```

In our Rust `FFrame`, `arg::<T>()` wraps `Step` (it also handles the `Code == null`
compiled-in/native path via `StepExplicitProperty`), and `finish()` is `P_FINISH`.

## 6. The actual thunks (verified by decompiling the Chivalry 2 binary)

| function          | params (in `Step` order)                                                                 |
|-------------------|-------------------------------------------------------------------------------------------|
| `execPrintString` | `WorldContextObject`(obj), **`InString`(FString)**, `bPrintToScreen`(bool), `bPrintToLog`(bool), `TextColor`(FLinearColor), `Duration`(float) |
| `execPrintWarning`| **`InString`(FString)** — *one* param, **no** WorldContextObject                          |
| `execPrintText`   | `WorldContextObject`(obj), **`InText`(FText)**, `bPrintToScreen`, `bPrintToLog`, `TextColor`, `Duration` |

Notes: UE 4.25 — `PrintString`/`PrintText` have **6** params and **no `Key`** (the `FName Key` arg
is 4.26+). Each thunk ends with `P_FINISH`, then calls the folded no-op body, then frees the
stepped `FString` (`FMemory::Free`) / releases the stepped `FText` shared-reference.

## 7. Why the earlier attempt crashed

The previous detour did:

```rust
let mut frame = unsafe { std::ptr::read(stack) }; // bitwise COPY of the FFrame
let message: FString = unsafe { frame.arg() };    // ONE step
... OriginalExecPrintString.call(context, stack, result);
```

Two independent bugs:

1. **Wrong argument.** One `Step` reads argument **0** = `WorldContextObject` (a `UObject*`), not
   `InString` (argument 1). It wrote an 8-byte object pointer into an `FString`-shaped buffer, so
   any later use read garbage.
2. **Double evaluation.** Copying the frame does **not** isolate anything: the copy's `Code` points
   at the *same* shared bytecode. Stepping the copy evaluates the real `WorldContextObject`
   expression (often a getter that reaches into the calling widget), and then
   `OriginalExec*.call()` ran the real thunk which evaluated **every** argument again. Re-entering
   UMG from the detour during a widget rebuild was the access violation.

You cannot "peek" at `InString` without first consuming argument 0, and you cannot let the original
run after you've stepped, because stepping *is* running the arguments.

## 8. The fix

Replicate the thunk faithfully on the **real** frame, then skip the no-op original:

```rust
let Some(frame) = (unsafe { stack.as_mut() }) else { return };
unsafe {
    let _world_context: *mut UObject = frame.arg(); // 0
    let message: FString             = frame.arg(); // 1  (owned; Drop frees via GMalloc)
    let _print_to_screen: u32        = frame.arg(); // 2
    let _print_to_log: u32           = frame.arg(); // 3
    let _text_color: [f32; 4]        = frame.arg(); // 4 (FLinearColor)
    let _duration: f32               = frame.arg(); // 5
    frame.finish();                                 // P_FINISH
    if let Ok(message) = message.copy_to_string() { sinfo!("[Blueprint PrintString] {message}"); }
    // no OriginalExec*.call(...): shipping callee is a no-op
}
```

Why this is correct:

- **Exactly once.** Every argument expression is evaluated a single time, by us — identical to what
  the original thunk would do. No double side effects.
- **Frame left consistent.** Stepping the real frame advances the real `Code`; `finish()` steps
  past `EX_EndFunctionParms`, so the VM resumes exactly where the original would have left it.
- **Skipping the original is safe** because the shipping callee is a folded no-op (we've already
  done the only real work: consume the args + advance `Code`).
- **Ownership / cleanup.** `arg::<T>()` zero-inits the destination first (a zeroed `FString`/`FText`
  is a valid empty one, required because `Step` *assigns* into it). The stepped `FString` is owned
  and its `Drop` frees the engine buffer via `globals().gmalloc()` — the same `FMemory::Free` the
  thunk uses. `FText` has no `Drop`, so `release_text()` releases its shared reference, mirroring
  the thunk's tail.
- **Native path.** If `Code == null` (function reached via `ProcessEvent`/`Invoke`, args already
  marshaled), `arg()` steps via `StepExplicitProperty` and `finish()` is a no-op — the same
  per-parameter consumption still applies, so the uniform code is correct on both paths.

### On macros vs. explicit code

A macro could generate these from a parameter list, but there are only three functions and their
shapes differ (PrintWarning has no context object; PrintText carries an `FText`). Explicit bodies
that mirror the decompiled `Step` order are easier to audit against the binary and make the "string
is argument 1" ordering impossible to miss, so we kept them explicit.

## 9. Resolved addresses / signatures (Chivalry 2 shipping, image base `0x140000000`)

- `FFrame::Step` — VA `0x141e1dfb0`, RVA `0x1e1dfb0`, proto `void Step(UObject* Context, void* Result)`.
  AOB: `48 8B 41 20 4C 8B D2 48 8B D1 44 0F B6 08 48 FF C0 48 89 41 20 41 8B C1`.
- `FFrame::StepExplicitProperty` — VA `0x141e1dfe0`.
- `execPrintString` VA `0x143648130`, `execPrintText` VA `0x143648310`, `execPrintWarning` VA
  `0x143648540` (these are resolved at runtime from `RegisterNatives`, not these static VAs; listed
  only for cross-referencing in Ghidra).

## 10. Validation (not yet run — see beads `ghidra_chiv-jat.4`)

Build from WSL via Windows Cargo:

```
cmd.exe /C "cd /d U:\Unchained\2026\UnchainedPlugin && cargo build"
```

Enable `blueprint_printstring_log`, load the plugin, trigger a Blueprint `PrintString` (including a
frontend/UMG path that previously crashed), and confirm the string is logged with no access
violation. Repeat for Warning/Text.
