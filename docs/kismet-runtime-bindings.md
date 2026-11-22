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

Reflection covers `UGameplayStatics::{GetAllActorsOfClass, LoadGameFromSlot,
SaveGameToSlot}`, `ATBLCharacter::{OnDamageTaken, OnKilled}`,
`ATBLPlayerController::{CanUseCharacter, CanUseLoadoutItem}`,
`UUserFeedbackAndBugReportsLibrary::GetGameInfo`,
`UTBLSystemLibrary::GetTBLGameMode`, `UTBLGameInstance::LoadFrontEndMap`,
`APlayerState::GetPlayerName`, and `UAssetRegistryHelpers::GetAssetRegistry`.
Existing detours, native signatures, resolver keys, and activation conditions
are unchanged. Chat readiness checks query each binding's backend explicitly.
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
| `/Script/TBL.TBLCharacter::OnDamageTaken` | `0x1c18ab0` | `0x18bffd0` | Instance; receiver, damage-event pointer; void |
| `/Script/TBL.TBLCharacter::OnKilled` | `0x1c18c00` | `0x18c0860` | Instance; receiver, death-event pointer; void |
| `/Script/TBL.TBLPlayerController::CanUseCharacter` | `0x1c671a0` | `0x19c5910` | Instance; receiver, return storage, subclass pointer; storage pointer |
| `/Script/TBL.TBLPlayerController::CanUseLoadoutItem` | `0x1c67320` | `0x19c5e40` | Instance; receiver, return storage, loadout pointer, item pointer; storage pointer |
| `/Script/TBL.UserFeedbackAndBugReportsLibrary::GetGameInfo` | `0x1cb5140` | `0x1aead90` | Static; FString return storage, world pointer; storage pointer |
| `/Script/TBL.TBLSystemLibrary::GetTBLGameMode` | `0x1c923f0` | `0x1a3ecd0` | Static; context pointer; game-mode pointer |
| `/Script/TBL.TBLGameInstance::LoadFrontEndMap` | `0x1c35c90` | `0x1945580` | Instance; receiver, FString pointer; bool in AL |
| `/Script/Engine.PlayerState::GetPlayerName` | `0x38728b0` | `0x356bb90` | Instance; receiver, FString return storage; storage pointer |
| `/Script/AssetRegistry.AssetRegistryHelpers::GetAssetRegistry` | `0x2f70b10` | `0x2f569b0` | Static; TScriptInterface return storage; storage pointer |

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

## Scope

This ports the address-source mechanism from `feature/exec-thunk-hooks`, not its
old macros, late-hook inventory, worker-thread fallback, print logging, or identity
overrides. Virtual/interface receivers and raw exec-thunk bindings remain separate
work. Do not substitute a shared shipping stub or an exec thunk for a native body.
Chat's virtual implementations remain scan-backed.
Asset-registry interface calls retain rust-ctables dispatch
and their interface-subobject receiver.

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
