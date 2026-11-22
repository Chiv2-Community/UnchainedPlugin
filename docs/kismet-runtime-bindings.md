# Reflection-backed native bindings

`unchained-macros-kismet` supplies the Kismet attribute, metadata validation,
native-body recovery, and address caching.
`unchained-macros-core` owns the generic resolver trait, selection filters, bindings,
patch application, and detour storage. UObject access stays in the game's model:
`src/unchained/blueprints/reflection.rs` implements the
Kismet `Reflection` trait and returns copied function/property metadata.

## Lifecycle

- Early startup resolves only Patternsleuth scan roots, initializes that backend's bindings, and
  attaches early hooks. Runtime descriptors never enter the missing-RVA scan loop.
- The `FEngineLoop::Init` detour initializes runtime bindings at entry and retries
  unresolved work at return. Both execute on the game thread, not a worker fallback.
- `Resolver<R>` and the binding/hook/patch registrations retain their backend type.
  The application supplies `ScanContext` or a non-Send `KismetContext`; core has
  no resolver-kind enum, callback-name registry, or initialization-phase policy.
- The host supplies an `ExecEnvironment` with the module base, original
  frame-step entries, and frame-code offset. The trait adapter does only
  class/function lookup and field reads. Kismet validates metadata, chooses
  parameter handling, decodes the native body, and logs successful resolution.
- Successful Kismet lookups are cached independently. Function publication is
  idempotent; all selected aliases are bound before runtime detours attach.
- Runtime patches retain host-owned completion state so successful edits are not
  repeated during retries. Hooks already attached are skipped.
- The same frozen selector snapshot filters both backends. Factories and activation
  conditions do not affect root selection.
- Direct calls use the resolved entry before attachment and the original trampoline
  afterward, bypassing that method's detour and listeners.
- With `blueprint_printstring_log`, early Step adapters observe original stepping.
  Runtime initialization then validates and attaches unique exec-wrapper listeners.
  This verified-wrapper integration is Steam-only; other storefronts are skipped.

Reflection covers `UGameplayStatics::{GetAllActorsOfClass, LoadGameFromSlot,
SaveGameToSlot}`, `ATBLCharacter::{OnDamageTaken, OnKilled}`,
`ATBLPlayerController::{CanUseCharacter, CanUseLoadoutItem}`,
`UUserFeedbackAndBugReportsLibrary::GetGameInfo`,
`UTBLSystemLibrary::GetTBLGameMode`, `UTBLGameInstance::LoadFrontEndMap`,
`APlayerState::GetPlayerName`, and `UAssetRegistryHelpers::GetAssetRegistry`.
Existing detours, native signatures, resolver keys, and activation conditions
are unchanged. Chat readiness checks query each binding's backend explicitly.
These bind native C++ bodies, **not** `UFunction::func` as a typed C++ method.
`kismet_server_identity_hooks` additionally binds the three KismetSystemLibrary
identity predicates. Its `force_kismet_is_*` features retain original calls and
deliberately override their results through detours, not listeners.

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
| `/Script/TBL.TBLCharacter::OnDamageTaken` | `0x1c18ab0` | `0x18bffd0` | Instance; receiver, damage-event pointer; void |
| `/Script/TBL.TBLCharacter::OnKilled` | `0x1c18c00` | `0x18c0860` | Instance; receiver, death-event pointer; void |
| `/Script/TBL.TBLPlayerController::CanUseCharacter` | `0x1c671a0` | `0x19c5910` | Instance; receiver, return storage, subclass pointer; storage pointer |
| `/Script/TBL.TBLPlayerController::CanUseLoadoutItem` | `0x1c67320` | `0x19c5e40` | Instance; receiver, return storage, loadout pointer, item pointer; storage pointer |
| `/Script/TBL.UserFeedbackAndBugReportsLibrary::GetGameInfo` | `0x1cb5140` | `0x1aead90` | Static; FString return storage, world pointer; storage pointer |
| `/Script/TBL.TBLSystemLibrary::GetTBLGameMode` | `0x1c923f0` | `0x1a3ecd0` | Static; context pointer; game-mode pointer |
| `/Script/TBL.TBLGameInstance::LoadFrontEndMap` | `0x1c35c90` | `0x1945580` | Instance; receiver, FString pointer; bool in AL |
| `/Script/Engine.PlayerState::GetPlayerName` | `0x38728b0` | `0x356bb90` | Instance; receiver, FString return storage; storage pointer |
| `/Script/AssetRegistry.AssetRegistryHelpers::GetAssetRegistry` | `0x2f70b10` | `0x2f569b0` | Static; TScriptInterface return storage; storage pointer |
| `/Script/Engine.KismetSystemLibrary::IsDedicatedServer` | `0x381a370` | `0x338b420` | Static; UObject context pointer; bool in AL |
| `/Script/Engine.KismetSystemLibrary::IsServer` | `0x381a630` | `0x338b9c0` | Static; UObject context pointer; bool in AL |
| `/Script/Engine.KismetSystemLibrary::IsStandalone` | `0x381a730` | `0x338ba40` | Static; UObject context pointer; bool in AL |

The ten parameterized thunks branch between `FFrame::Step` (`RVA 0x1ff40c0`) and
`StepExplicitProperty` (`RVA 0x1ff40f0`), advance the real frame past parameters,
and directly call the listed body. Save/load then release their temporary string;
actor enumeration also handles the output-array reference. The recovered body
RVAs match the previous signature-backed client cache.

`GetPlayerName` and `GetAssetRegistry` have only reflected return parameters.
Their thunks advance `FFrame::Code` inline, then call the native body without a
frame-step call. Resolution selects this mode only when `UFunction::NumParms`
minus its return parameter is zero, and verifies the inline advancement before
selecting the native call. The decoder receives `offset_of!(FFrame, code)` from
the host; it does not infer the native ABI from the metadata.
`GetPlayerName` retains its native custom-name virtual dispatch. The asset helper
retains explicit return storage and the returned interface-subobject pointer.

`LoadFrontEndMap` is registered in this binary despite being absent from the
local SDK header. Shared names such as `OnKilled` occur on multiple classes;
the concrete owner and native callee must both match.

`FEngineLoop::Init` is at RVA `0xa01720`, independently located through the
`FEngineLoop::Init` string and x64 unwind extent. Its receiver is passed in RCX;
the binding retains the upstream resolver's `int32 Init(FEngineLoop*)` ABI.

### Shipping print methods

The native-registration function at RVA `0x3810590` registers 242 methods from
table RVA `0x4fa2b60`. Its class constructor names
`/Script/Engine.KismetSystemLibrary`.

| Method | Exec thunk RVA | Body RVA | Verified behavior |
|---|---:|---:|---|
| PrintString | `0x381ee00` | `0xa016c0` | Six argument steps, shared return stub, temporary FString cleanup |
| PrintWarning | `0x381f210` | `0xa016c0` | One FString step, shared return stub, temporary FString cleanup |
| PrintText | `0x381efe0` | `0xabdd00` | Six argument steps, by-value FText retain/release, temporary FText cleanup |

The exported Wwise name on the shared return stub is an identical-code-folding
artifact, not ownership evidence. PrintText's body is cleanup-only, not the same
return stub: it releases the copied argument's shared reference. Its wrapper
separately releases the stepped temporary. Skipping either changes ownership.
The old branch's blanket description of all three bodies as a shared no-op is
not accurate for this binary.

The native PrintWarning declaration takes `const FString&`; a by-value Rust
observation must use an independent copy, never a shallow copy of engine storage.

## Scope

This ports the used address-source, print-diagnostic, and identity behavior from
`feature/exec-thunk-hooks`, not its old macros, late-hook inventory, worker-thread
fallback, or frame-consuming “observe” implementation.
Do not substitute a shared shipping stub or an exec thunk for a native body.
Chat's virtual implementations remain scan-backed.
Asset-registry interface calls retain rust-ctables dispatch
and their interface-subobject receiver.

## Entry and exit observation

```rust,ignore
#[unchained_macros_core::bind_func_entry_listener(UKismetSystemLibrary::PrintWarning)]
unsafe fn print_warning(message: &FString) {
    if let Ok(message) = message.copy_to_string() {
        log_blueprint_print("PrintWarning", &message);
    }
}
```

The borrow is intentional: a by-value FString would steal the engine's allocation.
An exit listener adds a borrowed result (`&()` for void) after the same inputs;
its independently owned input snapshots survive original thunk cleanup.
FString snapshots copy native storage; FText snapshots retain/release a native
shared reference. Neither wrapper-owned argument is dropped by the listener.

- One entry and one exit listener per method are supported alongside one native
  detour. Excess phase registrations fail preflight, not silent last-wins selection.
- Original exec thunks run exactly once. Top-level original Step completions
  expose initialized arguments before body execution; nested expression Steps
  are not counted as additional parameters. Frame/code state is never replayed.
- `#[kismet_exec_bindings]` requires explicit `#[exec_observe(verified)]`
  declarations. `dummied` marks verified no-op or cleanup-only bodies and rejects
  native detours. These markers are not callable native methods.
- Schema checks reject shared wrappers, mutable ref/out parameters, arrays and
  unsupported property kinds/layouts. Exact Step order and temporary lifetimes
  remain unsafe, binary-specific declaration obligations.
- Shared wrappers cannot use `FFrame::node` as an assumed callee identity.
  UE's `EX_CallMath` does not establish `CurrentNativeFunction` either. Such
  methods need an independently verified identity adapter and are rejected here.
- Core [listener contracts](native-bindings.md#observation-listeners) cover
  activation, callbacks, snapshots, reentry, original bypass and failure behavior.

### Parity coverage

| Reference behavior | Current implementation |
|---|---|
| PrintString, PrintWarning, PrintText | Unique exec-wrapper entry listeners; original execution and cleanup retained |
| Console, `blueprint.log`, exact-message filter | `blueprint_log` routing and restart-loaded `blueprint_filter.txt`; [configuration](logging.md) |
| Print feature gates | Opt-in `blueprint_printstring_log`; `kismet_diagnostics` also enables existing `kismet_log`; defaults unchanged |
| Identity observation | `kismet_server_identity_hooks`, independent of forced results |
| Forced identity predicates | Three separate `force_kismet_is_*` detour features |
| Native and virtual targets | Verified native bindings; virtual calls remain rust-ctables, with separately resolved concrete entries required for interception |
| Startup | Quiescent game-thread Init entry, unresolved retry at return; no worker reflection fallback |

Automatic reflected-vtable recovery is not implemented. The old branch's generic
virtual helper is not ABI evidence; unsupported indirect dispatch still fails
closed rather than guessing a receiver/slot. EGS exec-wrapper behavior remains
unvalidated and is not enabled by this integration.

## Validation

The initial three-method migration's isolated client/server startup checks used
the Steam binary identified above.

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

### Seven additional direct-call bindings

- Isolated Steam client/server runs used DLL SHA-256
  `a8ec29271ec24679beb20c4d6786878981d851e64e20ecfaaffc6e38f940e906`.
- Both roles recovered all ten reflected targets at the previous signature RVAs
  and attached all 42 hooks. The client exercised `LoadFrontEndMap` and its
  original-call bypass through frontend map loading.
- Fresh scans preserved 88 remaining baseline offsets plus `FEngineLoopInit`:
  89 cache entries, 63 roots, pattern batches `[101, 6, 66]`.
- Host macro/documentation suites: 300 tests passed.
- Windows plugin tests: 518 passed, 20 ignored with defaults; 509 passed,
  20 ignored with only `discord_integration,mod_management`.
- Optional-feature compilation and ordinary plugin documentation passed.
  Plugin rustdoc retains 14 unrelated warnings.
- Bare `--no-default-features` is blocked by existing unconditional Discord
  imports; Discord alone is blocked by unconditional mod-manager consumers.
  Those feature-gating issues are outside this migration.

### No-argument bindings

- Isolated Steam client/server runs used DLL SHA-256
  `60bc9847c352ef8120aca095d61f7993ba6f7450f68415121efb3dd9eaf5fd15`.
- Both roles recovered all twelve reflected targets at their previous signature
  RVAs and attached all 42 hooks without pending runtime initialization.
  The client exercised the asset-registry helper during frontend loading.
- Fresh scans preserved 86 remaining baseline offsets plus `FEngineLoopInit`:
  87 cache entries, 61 roots, pattern batches `[99, 6, 66]`.
- Host suites: 311 tests passed, including 17 generic decoder tests.
- Windows tests: 518 plugin tests and 17 decoder tests passed; 20 plugin tests
  ignored. The reduced-feature plugin suite passed 509 tests, with 20 ignored.
- Optional-feature compilation, plugin documentation, rendered binding docs,
  and Kismet crate documentation with warnings denied passed.
- These checks do not exercise player-name customization in gameplay or establish
  EGS compatibility.

### Reflection trait boundary

- `Reflection::find_function` returns `FunctionMetadata`, not a resolved native
  address. The adapter lives in `src/unchained/blueprints/reflection.rs`.
- The context owns its adapter and receives the executable environment explicitly.
  Both Send and Sync remain rejected by compile-fail tests.
- Host suites passed 321 tests. Windows passed 518 plugin tests and 28 Kismet
  tests, including real synthetic-thunk decoding, retry/cache behavior, and
  cross-backend trampoline sharing. Reduced-feature plugin tests passed 509 tests;
  both plugin configurations retain 20 ignored tests.
- Optional-feature compilation, plugin docs, and strict macro-crate docs passed.
- Isolated Steam client/server checks used DLL SHA-256
  `c6e3b05c8eef5dba8cf44b99d79a19fdbd6b61fd0faab22ef708152ee23e0bd6`.
  All twelve reflected addresses, 42 hooks, 61 scan roots, and 87 cache entries
  matched the previous checks. EGS remains unverified.

### Inventory and macro cleanup

- Inventories now store trait-object references directly. `Any` supertraits
  preserve safe typed downcasts without four duplicate erased-registration
  structs or duplicate erased pointers.
- Macro parsing and signature preparation no longer duplicate inherent impls,
  detour signatures, argument lists, or unused signature checks.
- Full default Windows expansion retains 332 inventory nodes; tokens match the
  previous expansion after normalizing inventory representation and build provenance.
- Host suites passed 323 tests. Windows passed 173 backend tests, 518 default
  plugin tests, and 509 reduced-feature plugin tests. Optional-feature compilation,
  plugin docs, and strict macro-crate docs passed.
- Isolated Steam client/server checks used DLL SHA-256
  `97d5702f535ad7f6b7d3fadba58cf31740fb1054ab71188c131c72b310e657b9`.
  Both roles retained all twelve reflected addresses, 42 hooks, 61 scan roots,
  and 87 cache entries. The server's cold scan exited with the expected restart
  code `-67` (`189`); its warm run completed runtime initialization.
  EGS remains unverified.

### Coordinated entry/exit listeners

- Final suites passed 382 host tests and 219 Windows backend tests. Plugin suites
  passed 522 default tests, 513 reduced-feature tests, and 524 tests with print,
  identity, and all forced-result features. Plugin suites retain 21 ignored tests.
  Optional-feature compilation, plugin docs, strict macro docs, and release builds
  passed.
- Independent review/fix passes addressed callback-publication safety, physical
  activation retry, attachment-condition reentry, panic-hook/logger containment,
  generated-local hygiene, duplicate registration/status ownership, and VM bool
  decoding. The final source review found no remaining actionable cleanup.
- Production feature DLL SHA-256:
  `0869364b8558b8f1671de61845268f649af374ba1e9f43baf05cae09d516cb43`.
- Isolated Steam client/server probes used an instrumented DLL:
  `cc9a3a9782c6ddb9928bc0bbc4744f87472816761fada213bdcee3b57d84f3c5`.
  Fresh scans used the preceding probe artifact
  `48a6215104aef01c0363147ce5479b3f6d1309d24718c51451642f6a7ecab5bd`;
  both preserved the previous cached addresses. Diagnostics add one scan root
  and cache entry for existing Kismet execution-message logging.
- Each role observed five visible print messages exactly once and suppressed one
  exact filter match. Exit listeners read four warning inputs, one string input,
  and one text input after original thunk cleanup; retained FText references
  balanced `2 → 2`.
- Real `ProcessEvent` frames exercised compiled-in stepping, then test-only code
  injection exercised Unicode bytecode and nested `MakeLiteralString` evaluation
  on the same frame. Original stepping counted exactly one and two calls
  respectively; code advancement and the compiled-in property chain were checked.
  No frame was copied or replayed. Probe code is not included in production.
- Identity checks compared original-call bypass with forced results and observed
  each final native result once. The probe disabled overrides during engine
  initialization and enabled them for these checks; this is not gameplay/role
  compatibility validation for arbitrary combinations of forced predicates.
- Shared exec wrappers, mutable ref/out schemas, automatic reflected-vtable
  recovery, and generic native-reference exit snapshots remain explicitly
  unsupported. EGS print observation remains disabled pending binary validation.
