# Contributing to Unchained Plugin

## Contents

- [Creating game hooks](#creating-game-hooks)
  - [Find the native owner and ABI](#find-the-native-owner-and-abi)
  - [Create a resolver](#create-a-resolver)
    - [Byte patterns](#byte-patterns)
    - [Platform selection](#platform-selection)
    - [Custom searches](#custom-searches)
  - [Declare a native binding](#declare-a-native-binding)
  - [Declare vtable methods](#declare-vtable-methods)
    - [Inherited vtables](#inherited-vtables)
  - [Create the detour](#create-the-detour)
  - [Validate hooks](#validate-hooks)
- [Creating commands](#creating-commands)
- [Creating vote types](#creating-vote-types)
- [Creating subscribers](#creating-subscribers)
- [State and game-thread work](#state-and-game-thread-work)
- [Logging](#logging)

Extend the plugin through native hooks, commands, votes, and event subscribers.

- Follow existing modules and feature gates.
- Format `unchained-macros-*` with `bash scripts/fmt-macros.sh`. CI runs the same
  command with `--check`. These crates use standard rustfmt with a 120-column target.
- Keep native declarations separate from plugin behavior.
- Use the [build instructions](README.md#building-the-windows-dll-on-linux) for Linux cross-builds.

## Creating game hooks

Resolve a native address, declare its binding, then attach a plugin replacement.

- **Resolver and binding:** `src/Chivalry2/*`, beside the native owner.
- **Detour and instruction patches:** `src/unchained/patches/*`, grouped by purpose.
- **Logging-only hooks:** `src/unchained/patches/diagnostics.rs`.
- Use `runtime_bindings` and `bind_func_detour`; call the native binding for
  original-call bypass.

### Find the native owner and ABI

Establish the native contract before writing Rust declarations.

- Use source, debug symbols, and disassembly to verify arguments and returns.
- Follow the [source layout](docs/engine-sources.md):
  - Preserve native casing and header paths, including `Runtime`, `ThirdParty`,
    `CoreUObject`, and `TBL`.
  - Omit `Public`, `Private`, and `Classes`.
  - Put native game-plugin types under `src/Chivalry2/Plugins/`.
- Add new files to their parent `mod.rs`.
- Put resolvers before native types and bindings.
- Keep static methods static; declare instance receivers explicitly.
- Use C++ spelling for native bindings and virtual methods: `GetWorld`,
  `UDPEcho`, and `GetPost_GetMotd`, not snake_case.
- Add descriptive suffixes only to disambiguate overloads, such as
  `FindFileInPakFiles_Static` and `Serialize_WithTime`. Constructors use the
  type name; native C++ destructors use `Destructor`.
- Preserve destructor ABI details, including deleting flags and pointer returns.
- Preserve native acronyms: use `TBLFoo`, `GetTBLGameMode`, and `UTBLGameInstance`,
  never `TblFoo` or `GetTblGameMode`.
- Keep Rust-only helpers and trait implementations idiomatic Rust.
- Remove comments that only repeat signatures; retain ABI, ownership, safety,
  and source-evidence notes.
- Include hidden return-storage arguments.
- Use raw pointers unless Rust reference guarantees are established.
- Keep opaque types opaque; do not invent layouts.

### Create a resolver

Reuse an existing resolver or declare one beside the binding.

- Name project-defined method resolvers `Class__Method`; use `Function` for free
  functions and `Owner__Value` or `Value` for globals. Use `Class__Destructor`
  for destructors.
- Keep native spelling and capitalization, including `TBL` and `GetPost_*`.
- Upstream patternsleuth resolver types retain their upstream names.
- Resolver identifiers are cache/registration keys. When explicitly renaming one,
  update readiness checks, patches, tests, and baseline entries; old cached keys
  do not initialize the renamed binding.
- Resolve function entries, not interior instructions, for callable bindings.
- Wildcard relocations and build-dependent bytes with `??`.
- Verify each supported build; reject missing or conflicting addresses.
- Examples named `Example*` show syntax, not verified game signatures.
- Existing examples illustrate alternatives; do not register duplicate targets.

#### Byte patterns

Use `define_pattern_resolver!` for byte scans.

- `Simple` is the default: require one unique matched address.
- `Call` follows a four-byte relative displacement; mark its start with `|`.
- `First` selects the first match without rejecting ambiguity.
- `XrefFirst` / `XrefLast` use the first string pattern, find references, and
  select the first/last containing root function.
- Use ordering-based modes only when verified against supported builds.

Existing UDP-ping resolver:

```rust
define_pattern_resolver!(FUDPPing__UDPEcho, [
    "48 89 5C 24 20 55 56 57 48 81 EC 90 00 00 00 49 8B D8 0F 29 B4 24 80 00 00 00 48 8B E9"
]);
```

Call-target and string-reference examples:

```rust
define_pattern_resolver!(ExampleCallTarget, Call, [
    "E8 | ?? ?? ?? ?? 4C 39 ?8 74 3?"
]);

define_pattern_resolver!(ExampleStringOwner, XrefFirst, [
    patternsleuth::resolvers::unreal::util::utf8_pattern("Unique native diagnostic")
]);
```

#### Platform selection

Select patterns for each storefront build.

- Names: `STEAM`, `EGS`, `XBOX`, `OTHER`.
- [`src/platform.rs`](src/platform.rs) supplies selector state through
  `ResolverSelector::current()`. There are no project wrapper macros.
- `fallback = OTHER` advances only when a dictionary key is absent, not when a
  declared signature fails to match. The first declared entry wins.
- Each dictionary must explicitly include `STEAM` and `EGS`, as required by
  the enum's `#[resolver_selection_key(require = [STEAM, EGS], fallback = OTHER)]` configuration.
- Use `PLATFORM: None` to disable scanning, bindings, hooks, and patches for that
  storefront, even when `OTHER` has patterns. Empty lists are compile errors.
  Plain pattern lists apply to all storefronts.

```rust
define_pattern_resolver!(ExampleFunction, {
    PlatformType::STEAM: ["48 89 5C 24 ?? 57"],
    PlatformType::EGS: ["40 53 48 83 EC ??"],
    PlatformType::OTHER: ["48 8B C4 48 89 58 ??"]
});

define_pattern_resolver!(ExampleSteamOnly, {
    PlatformType::STEAM: ["48 89 5C 24 ?? 57"],
    PlatformType::EGS: None,
    PlatformType::XBOX: None,
    PlatformType::OTHER: None,
});
```

#### Custom searches

Use BinaryBloodhound to compose searches and recover function entries.

- `define_custom_resolver!` requires a no-input matcher yielding exactly one
  `u64` address.
- Use `.root_functions()` for interior matches, `.addresses()` for address
  values, and `.ensure_one()` to reject missing or conflicting results.
- Use `using crate::PlatformType, |platform| { ... }` for
  matcher construction from the frozen selector snapshot.
  This selects the search; it does not register a platform restriction.
- For inline async scans or mixed-mode resolvers, see the
  [resolver reference](unchained-macros-patternsleuth/resolvers.md#macros).
- See [custom resolver examples](unchained-macros-patternsleuth/resolvers.md#custom-resolvers)
  for error handling and platform-aware searches.

```rust
use binary_bloodhound::BinaryMatcher;

define_custom_resolver!(ExampleRootFunction, {
    BinaryMatcher::patterns(["48 89 5C 24 ?? 57"])?
        .root_functions()
        .addresses()
        .ensure_one()
});
```

### Declare a native binding

Declare an empty `unsafe fn` inside `#[runtime_bindings] impl Type`.

- Pass the resolver's type path to `bind_resolved_func`; do not rename its import.
- `self::ResolverName` explicitly selects the current module's resolver.
  Qualification is optional when unambiguous; `Self::` instead refers to the type.
- Use visibility accessible to the detour, usually `pub(crate)`.
- For instance methods, declare `self: *mut Self` or another verified receiver.
- The generated ABI is Windows x64 `extern "C"`.
- Macros do not infer C++ layouts or verify signatures against the binary.
- Startup initializes bindings automatically; uninitialized calls panic.

Static UDP-ping binding, with an opaque callback type:

```rust
use crate::Chivalry2::Engine::FString;
use std::ffi::c_void;

pub type FIcmpEchoResultCallback = c_void;

pub struct FUDPPing {
    _private: (),
}

#[unchained_macros_core::runtime_bindings]
impl FUDPPing {
    #[unchained_macros_core::bind_resolved_func(FUDPPing__UDPEcho)]
    pub(crate) unsafe fn UDPEcho(
        endpoint: *const FString,
        timeout: f32,
        #[native_by_value] callback: *mut FIcmpEchoResultCallback,
    ) {}
}
```

Other binding sources:

- **Patternsleuth:** pass a typed resolver declaration. Adapt upstream resolvers
  with `define_custom_resolver!(Name, delegate upstream::Resolver)` first.
- **Provider offsets:** declare `define_offset_resolver!(GetPost_GetMotd)`, then
  use `bind_resolved_func(GetPost_GetMotd)` inside `runtime_bindings`.
  Populate that exact key in `OffsetProvider` with
  an RVA, not an absolute address. See the
  [OpenAPI bindings](src/Chivalry2/Plugins/OnlineSubsystemPlayFab/OpenAPIMerged/OpenAPIClientApi.rs).
- **Native globals:** mirror every C++ static/global used here as a Rust static,
  with native spelling and declaring-header placement. Use
  `bind_resolved_value(Resolver, Address)` for inline storage, or `Value` for a
  pointer slot. Read explicitly at call sites:
  `unsafe { GMalloc.read().Malloc(size, alignment) }`.
  rust-ctables handles vtable dispatch; no global forwarders are generated.
  No owning handles, slot borrows,
  cached pointees, or binding locks. Native synchronization still applies.
  These are not detour targets; plugin-owned state is separate. See
  [native statics](unchained-macros-core/README.md#native-globals).

### Declare vtable methods

Use `rust_ctables::cpp_struct` for calls dispatched through a native object's vtable.

- Keep vtable declarations with their native owner under `src/Chivalry2/*`.
- Verify the vptr location, slot offsets, receiver, and ABI for supported builds.
- `offset(...)` is a **byte offset within the vtable**, not a slot index or RVA.
  Slots are eight bytes on Windows x64; declarations advance one slot each.
- Use explicit offsets to skip unmodeled entries.
- Generated methods are unsafe and use the object's current vtable.
- Keep the object and table alive; obey native aliasing and thread requirements.
- Vtable dispatch needs no pattern resolver, but it does not initialize or
  validate the object.

Existing `FUniqueNetId` entries at `0x28` and `0x30`:

```rust
use crate::Chivalry2::Engine::FString;

#[rust_ctables::cpp_struct]
pub struct FUniqueNetId {
    #[vtable(
        offset(0x28);
        IsValid(&self) -> bool;
        ToString(&self, output: *mut FString) -> *mut FString;
    )]
    vtable: *const (),
}

unsafe fn read_id(id: &FUniqueNetId) -> Option<String> {
    if !unsafe { id.IsValid() } {
        return None;
    }
    let mut output = FString::new();
    unsafe { id.ToString(&mut output) };
    Some(output.to_string())
}
```

- `ToString` explicitly models caller-provided return storage.
- `&self` supplies the native `this` pointer.
- Use `self: *mut Self` for raw mutable receivers when a Rust borrow is
  inappropriate; see [FOutputDevice](src/Chivalry2/Engine/Runtime/Core/Misc/OutputDevice.rs).
- See [FUniqueNetId](src/Chivalry2/Engine/Runtime/CoreUObject/UObject/CoreOnline.rs)
  for a native-table fixture test and
  [FInternetAddr](src/Chivalry2/Engine/Runtime/Sockets/IPAddress.rs) for a slot-15 call.

#### Inherited vtables

Extend the inherited primary vtable rather than adding a second vptr.

- Put `#[inherit]` and `#[vtable(...)]` on the embedded base field.
- Offsets remain relative to the vtable start.
- Model base layout and subobject placement accurately; do not guess adjustments
  for multiple inheritance.
- The generated methods forward through the inherited vptr.

Existing `UObject::GetWorld` binding at byte offset `0x150`:

```rust
use crate::Chivalry2::Engine::{UObjectBaseUtility, UWorld};

#[rust_ctables::cpp_struct]
pub struct UObject {
    #[inherit]
    #[vtable(
        offset(0x150);
        GetWorld(&self) -> *mut UWorld;
    )]
    pub uobject_base_utility: UObjectBaseUtility,
}
```

### Create the detour

Target the binding method from a functional module under `src/unchained/patches/`.

- Put detours after imports, before patches, helpers, state, and tests.
- Use an unsafe free function; declare instance receivers as the first argument.
- Use snake_case for detour function and parameter names, even when the native
  binding uses C++ spelling.
- The macro checks argument types, return types, and lifetime relationships
  against the binding at compile time.
- Calling the binding invokes the original, bypassing this replacement.
- Assume startup initialized required bindings.
- A generated vtable method is not a `bind_func_detour` target. To intercept a
  concrete virtual implementation, resolve its entry and declare a separate
  `runtime_bindings` method. Calling through the vtable may re-enter the hook;
  use the resolver-backed binding for original calls.
- Native Patternsleuth and Kismet bindings support a replacement, an entry
  listener, and an exit listener. Kismet `#[dummy]` declarations support entry only.
- Only one detour may own a native address. For handwritten registration, pass the
  verified target and typed replacement to `DetourRegistration::new`; see the
  [core example](unchained-macros-core/README.md#handwritten-registrations).
- The condition defaults to `|| true`; activation also requires global
  auto-activation. Conditions run during attachment, not on each call.
- Preserve native thread, ownership, pointer, and callback-lifetime rules.
- Do not let Rust panics unwind through native callers.
- Match binding/replacement feature gates; gate whole modules when appropriate.
- Keep `register_patch` instruction patches here too; keep resolvers in the native model.
- See [binding and trampoline contracts](docs/native-bindings.md).

Instruction patches import their resolver type and use `register_patch` on a
factory or immutable `Patch` static. An unqualified patch requires an unrestricted
resolver; an explicit storefront cannot be disabled by the resolver:

```rust
#[register_patch(AllowAlternateBackend, crate::PlatformType::STEAM)]
static ALTERNATE_BACKEND_STEAM: Patch = Patch::bytes(0x4D, &[0x90, 0xE9]);
```

Patch names are item names. Coverage is grouped by resolver: its registered
patches must jointly cover enabled required storefronts. An EGS `None` resolver
entry removes the EGS requirement. Coverage is validated before scanning/writing;
patch selections do not narrow other consumers of the resolver.
Use `fn patch_name(resolved_address: u64) -> Patch` for address-dependent bytes
or runtime `.when(condition)` gates. Existing detours inherit resolver availability.

Alternative replacement for the existing UDP hook:

```rust
use crate::Chivalry2::Engine::{FString, FUDPPing, FIcmpEchoResultCallback};
use crate::tools::hook_globals::cli_args;

#[unchained_macros_core::bind_func_detour(FUDPPing::UDPEcho, || !cli_args().is_server())]
unsafe fn log_udp_echo(
    endpoint: *const FString,
    timeout: f32,
    callback: *mut FIcmpEchoResultCallback,
) {
    sdebug!("UDP ping requested with timeout {}", timeout);
    unsafe { FUDPPing::UDPEcho(endpoint, timeout, callback) };
}
```

### Validate hooks

Test replacement behavior, not native signatures, layouts, or registrations.

- Add behavioral tests for the replacement.
- Run `cargo test --locked --lib` on Windows with affected features enabled
  and disabled. Linux needs cargo-xwin and a Windows test runner such as Wine.
- Verify addresses, ABI, and behavior in supported game builds; unit tests
  cannot prove native ABI correctness.

## Creating commands

Implement `Command<Args>` and register it with the command subscriber.

- Add the implementation under `src/modules/`; declare it in `src/modules/mod.rs`.
- Derive `clap::Parser` for arguments; it also supplies `CommandFactory`.
- Choose the minimum `PermissionFlags` required.
- Override `required_source` when a command must be source-restricted.
- Register server commands in `src/features/events.rs::initialize_subscribers`.
- Add client-safe commands separately in `initialize_client_commands`.
- Register Discord-dependent commands during Discord initialization in
  `src/features/discord.rs`.

```rust
use async_trait::async_trait;
use clap::Parser;
use crate::commands::Command;
use crate::events::models::{CommandRequest, PermissionFlags};

#[derive(Parser, Clone)]
#[command(name = "example-log", about = "Write a diagnostic message")]
pub struct ExampleLogArgs {
    pub message: String,
}

pub struct ExampleLogCommand;

#[async_trait]
impl Command<ExampleLogArgs> for ExampleLogCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::ADMIN
    }

    async fn execute(&self, args: ExampleLogArgs, _request: &CommandRequest) {
        sinfo!("{}", args.message);
    }
}
```

Registration inside `initialize_subscribers`, while its subscriber lock is held:

```rust
command_subscriber.register(ExampleLogCommand);
```

## Creating vote types

Implement `VoteType<Args>` and register it with `VotingState`.

- Add the implementation under `src/modules/votes/`; update its `mod.rs`.
- Derive `Parser` and `Clone` for arguments; derive `Clone` for vote logic.
- Implement the title, description, thresholds, success action, and `clone_box`.
- Use `check_prerequisites` to reject invalid requests before starting a vote.
- Register with `state.register(MyVote)` in
  `src/features/events.rs::initialize_vote_commands`.
- See [speed votes](src/modules/votes/votespeed.rs) for argument handling and
  the [vote API](src/modules/vote.rs) for prerequisite checks and thresholds.

Existing end-map vote pattern:

```rust
use clap::Parser;
use crate::commands::NATIVE_COMMAND_QUEUE;
use crate::modules::vote::{ErasedVoteType, VoteType, VoteTypeHandler};

#[derive(Parser, Clone)]
#[command(name = "example-next-map")]
pub struct ExampleNextMapArgs;

#[derive(Clone)]
pub struct ExampleNextMapVote;

impl VoteType<ExampleNextMapArgs> for ExampleNextMapVote {
    fn title(&self) -> String { "End match".into() }
    fn description(&self) -> String { "End the current match".into() }
    fn vote_description(&self, _args: ExampleNextMapArgs) -> String {
        self.description()
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.6 }
    fn min_votes_required_ratio(&self) -> f32 { 0.5 }
    fn on_success(&self, _args: ExampleNextMapArgs) {
        NATIVE_COMMAND_QUEUE.lock().unwrap().push("tbsendgame 1".into());
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}
```

Registration inside `initialize_vote_commands`, before constructing shared state:

```rust
state.register(ExampleNextMapVote);
```

## Creating subscribers

Implement `Subscriber<T>` to consume events or periodic ticks.

- Add a module under `src/modules/` and declare it in `mod.rs`.
- Use `GameEvent` for gameplay events or
  `crate::events::broadcast::BroadcastMessage` for outgoing messages.
- Give each subscriber a unique `identifier`.
- Implement `on_event`; override `on_tick` only when periodic work is needed.
- Subscribe to the matching bus in `src/features/events.rs::initialize_subscribers`.
- Check subscription errors; duplicate identifiers are rejected.
- Keep handlers brief; slow work delays other subscribers.

```rust
use async_trait::async_trait;
use crate::events::bus::Subscriber;
use crate::events::models::GameEvent;

pub struct MapLogSubscriber;

#[async_trait]
impl Subscriber<GameEvent> for MapLogSubscriber {
    fn identifier(&self) -> &'static str {
        "MapLogSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        if let GameEvent::MapChangeEvent(change) = event {
            sinfo!("Map changed to {}", change.new_map_name);
        }
    }
}
```

Registration inside `initialize_subscribers`:

```rust
game_event_bus.subscribe(Box::new(MapLogSubscriber)).await
    .expect("MapLogSubscriber must be registered once");
```

## State and game-thread work

Keep native access on the correct thread and shared state explicit.

- Inject publishers and shared state through constructors; see
  [stats commands and subscribers](src/modules/stats.rs).
- Reuse existing hook state; add globals only when callback access requires them.
- Queue native work with
  `unchained::patches::unchained_integration::game_thread::run_on_game_thread`.
- Capture owned data; look up native objects inside the queued job.
- Do not hold locks while waiting for game-thread work.
- Use `NATIVE_COMMAND_QUEUE` for native console commands, as in the vote example.

## Logging

Use the shared logging macros rather than `println!`.

- Use specific messages; omit secrets such as passwords and tokens.
- See [emitting diagnostics](docs/logging.md#contributor-guide-emitting-diagnostics)
  for macros and function metadata.
- See [target levels](docs/logging.md#set-log-levels) and
  [structured logging](docs/logging.md#structured-logging) for configuration.
- See [log locations](docs/logging.md#log-location) and
  [chat logging](docs/logging.md#chat-logging) for output files and chat encoders.

```rust
sdebug!("Loading save game from slot {} (user index {})", slot, index);
```
