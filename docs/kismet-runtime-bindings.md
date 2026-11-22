# Reflection-backed native bindings

`unchained-macros-kismet` supplies the Kismet attribute and exec-thunk decoder.
`unchained-macros-core` owns the generic resolver trait, selection filters, bindings,
patch application, and detour storage. UObject access stays in the game's model
and `src/unchained/reflection.rs`.

## Lifecycle

- Early startup resolves only Patternsleuth scan roots, initializes that backend's bindings, and
  attaches early hooks. Runtime descriptors never enter the missing-RVA scan loop.
- The `FEngineLoop::Init` detour initializes runtime bindings at entry and retries
  unresolved work at return. Both execute on the game thread, not a worker fallback.
- `Resolver<R>` and the binding/hook/patch registrations retain their backend type.
  The application supplies `ScanContext` or a non-Send `KismetContext`; core has
  no resolver-kind enum, callback-name registry, or initialization-phase policy.
- Successful Kismet lookups are cached independently. Function publication is
  idempotent; all selected aliases are bound before runtime detours attach.
- Runtime patches retain host-owned completion state so successful edits are not
  repeated during retries. Hooks already attached are skipped.
- The same frozen selector snapshot filters both backends. Factories and activation
  conditions do not affect root selection.
- Direct calls use the resolved entry before attachment and the original trampoline
  afterward, even when the detour is disabled.

The initial migration covers `UGameplayStatics::GetAllActorsOfClass`,
`LoadGameFromSlot`, and `SaveGameToSlot`. The latter two keep their existing
diagnostic detours; the first is a direct binding without a detour.
These bind native C++ bodies, **not** `UFunction::func` as a typed C++ method.

## Binary evidence

Read-only inspection of the installed Steam Windows x64 executable:

- Steam app `1824220`, installed build `21012305`; manifest reports no pending build.
- Path: `TBL/Binaries/Win64/Chivalry2-Win64-Shipping.exe`.
- SHA-256: `d82b5eaa122aaee15366499bbce34e325741fecd06d6b7b5490fc650b5afb48b`.
- Preferred image base: `0x140000000`.
- Inspection used the Binary Ninja view opened directly from that installed file.
  This identifies the available target; it is not independent proof of Steam's
  current public release or EGS compatibility.

| Reflected function | Exec thunk RVA | Native body RVA | Native ABI |
|---|---:|---:|---|
| `/Script/Engine.GameplayStatics::GetAllActorsOfClass` | `0x37b2d20` | `0x32ddef0` | Static; context pointer, class pointer, output-array pointer; void |
| `/Script/Engine.GameplayStatics::LoadGameFromSlot` | `0x37b4ef0` | `0x32ea100` | Static; FString reference, i32 index; object pointer |
| `/Script/Engine.GameplayStatics::SaveGameToSlot` | `0x37b6c20` | `0x32f20e0` | Static; object pointer, FString reference, i32 index; bool in AL |

Each thunk branches between `FFrame::Step` (`RVA 0x1ff40c0`) and
`StepExplicitProperty` (`RVA 0x1ff40f0`), advances the real frame past parameters,
and directly calls the listed body. Save/load then release their temporary string;
actor enumeration also handles the output-array reference. The recovered body
RVAs match the previous signature-backed client cache.

`FEngineLoop::Init` is at RVA `0xa01720`, independently located through the
`FEngineLoop::Init` string and x64 unwind extent. Its receiver is passed in RCX;
the binding retains the upstream resolver's `int32 Init(FEngineLoop*)` ABI.

## Scope

This ports the address-source mechanism from `feature/exec-thunk-hooks`, not its
old macros, late-hook inventory, worker-thread fallback, print logging, or identity
overrides. Virtual/interface receivers and raw exec-thunk bindings remain separate
work. Do not substitute a shared shipping stub or an exec thunk for a native body.

## Validation

Isolated client/server startup checks used the Steam binary identified above.

- DLL SHA-256: `671ee698e1bdf65b0406a11a02fea2ad4e63fb1ed58a127cc89d06d85c674f4a`.
- Fresh scans preserved all 95 remaining baseline offsets and added `FEngineLoopInit`.
  The three migrated addresses were recovered through live reflection and matched
  their previous signature RVAs exactly.
- Scan cache: 96 entries; 70 roots; pattern batches `[111, 6, 66]`.
  The init resolver adds scan work despite the lower root count.
- Both roles reached runtime initialization and attached 42 hooks, including the
  new engine-init hook and the existing save/load diagnostic hooks.
- Generic tests cover typed discovery, independent selection, retry behavior,
  patch completion state, and original-call bypass for early and late aliases
  across different backend types.
- Host suites, Windows macro/plugin tests, optional/no-registration checks,
  documentation builds, and 32 rust-analyzer expansions passed.

These are startup checks, not gameplay coverage or repeated performance measurements.
EGS live validation remains unavailable.
