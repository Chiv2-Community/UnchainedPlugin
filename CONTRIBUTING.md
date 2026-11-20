# Contributing to Unchained Plugin

Extend the plugin through native hooks, commands, votes, and event subscribers.

- Follow existing modules and feature gates.
- Keep native declarations separate from plugin behavior.
- Use the [build instructions](README.md#building-the-windows-dll-on-linux) for Linux cross-builds.

## Creating game hooks

Resolve a native address, declare its binding, then attach a plugin replacement.

- **Resolver and binding:** `src/chivalry2/*`, beside the native owner.
- **Detour and instruction patches:** `src/unchained/patches/*`, grouped by purpose.
- **Logging-only hooks:** `src/unchained/patches/diagnostics.rs`.
- Use `runtime_bindings` and `bind_func_detour`, not legacy `CREATE_HOOK!` or
  `bind_resolved_func_detour`.

### Find the native owner and ABI

Establish the native contract before writing Rust declarations.

- Use source, debug symbols, and disassembly to verify arguments and returns.
- Follow the [source layout](docs/engine-sources.md):
  - Preserve `runtime`, `third_party`, and native header paths.
  - Omit `Public`, `Private`, and `Classes`.
  - Put native game-plugin types under `src/chivalry2/plugins/`.
- Add new files to their parent `mod.rs`.
- Put resolvers before native types and bindings.
- Keep static methods static; declare instance receivers explicitly.
- Include hidden return-storage arguments.
- Use raw pointers unless Rust reference guarantees are established.
- Keep opaque types opaque; do not invent layouts.

### Create a resolver

Reuse an existing resolver or declare one beside the binding.

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
define_pattern_resolver!(UDPEcho, [
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
- The current platform's patterns and `OTHER` are combined.
- `OTHER` is shared coverage, not a fallback after failed matches.
- Pattern selection does not restrict hook installation.
- Use `define_single_platform_pattern_resolver!` to restrict a resolver and
  its hooks/patches to one platform.

```rust
define_pattern_resolver!(ExampleFunction, {
    STEAM: ["48 89 5C 24 ?? 57"],
    EGS: ["40 53 48 83 EC ??"],
    OTHER: ["48 8B C4 48 89 58 ??"]
});

define_single_platform_pattern_resolver!(STEAM, ExampleSteamOnly, [
    "48 89 5C 24 ?? 57"
]);
```

#### Custom searches

Use BinaryBloodhound to compose searches and recover function entries.

- `define_custom_resolver!` requires a no-input matcher yielding exactly one
  `u64` address.
- Use `.root_functions()` for interior matches, `.addresses()` for address
  values, and `.ensure_one()` to reject missing or conflicting results.
- Use `|platform| { ... }` for platform-dependent matcher construction.
  This selects the search; it does not register a platform restriction.
- For inline async scans or mixed-mode resolvers, see the
  [resolver reference](src/resolvers/README.md#macros).
- See [custom resolver examples](unchained-macros/README.md#custom-resolvers)
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
- Use visibility accessible to the detour, usually `pub(crate)`.
- For instance methods, declare `self: *mut Self` or another verified receiver.
- The generated ABI is Windows x64 `extern "C"`.
- Macros do not infer C++ layouts or verify signatures against the binary.
- Startup initializes bindings automatically; uninitialized calls panic.

Static UDP-ping binding, with an opaque callback type:

```rust
use crate::chivalry2::engine::FString;
use std::ffi::c_void;

pub type TFunctionFIcmpEchoResult = c_void;

pub struct FUDPPing {
    _private: (),
}

#[unchained_macros::runtime_bindings]
impl FUDPPing {
    #[unchained_macros::bind_resolved_func(UDPEcho)]
    pub(crate) unsafe fn udp_echo(
        endpoint: *mut FString,
        timeout: f32,
        callback: *mut TFunctionFIcmpEchoResult,
    ) {}
}
```

Other binding sources:

- **Patternsleuth:** pass a full resolver type path, such as
  `patternsleuth::resolvers::unreal::game_loop::UGameEngineTick`.
- **Request registry:** use `bind_resolved_request_function(GetPost_GetMotd)`
  inside `runtime_bindings`. Populate that exact key in `OffsetRegisty` with
  an RVA, not an absolute address. See the
  [OpenAPI bindings](src/chivalry2/plugins/online_subsystem_playfab/open_api_merged/open_api_client_api.rs).
- **Native globals:** use `bind_resolver_value(Resolver, Address)` for an inline
  object's address, or `bind_resolver_value(Resolver, Value)` to read a `Copy`
  value from a slot. These are not detour targets. See
  [native value accessors](unchained-macros/README.md#required-native-values).

### Create the detour

Target the binding method from a functional module under `src/unchained/patches/`.

- Put detours after imports, before patches, helpers, state, and tests.
- Use an unsafe free function; declare instance receivers as the first argument.
- The macro checks argument types, return types, and lifetime relationships
  against the binding at compile time.
- Calling the binding invokes the original, bypassing this replacement.
- Only one detour may own a native address.
- The condition defaults to `ACTIVE`; activation also requires global
  auto-activation. Conditions run during attachment, not on each call.
- Preserve native thread, ownership, pointer, and callback-lifetime rules.
- Do not let Rust panics unwind through native callers.
- Match binding/replacement feature gates; gate whole modules when appropriate.
- Keep `CREATE_PATCH!` instruction patches here too; keep resolvers in the native model.
- See [binding and trampoline contracts](docs/native-bindings.md).

Alternative replacement for the existing UDP hook:

```rust
use crate::chivalry2::engine::{FString, FUDPPing, TFunctionFIcmpEchoResult};
use crate::tools::hook_globals::cli_args;

#[unchained_macros::bind_func_detour(FUDPPing::udp_echo, || !cli_args().is_server())]
unsafe fn log_udp_echo(
    endpoint: *mut FString,
    timeout: f32,
    callback: *mut TFunctionFIcmpEchoResult,
) {
    sdebug!("UDP ping requested with timeout {}", timeout);
    unsafe { FUDPPing::udp_echo(endpoint, timeout, callback) };
}
```

### Validate hooks

Test resolution and behavior, not redundant typed-pointer assignments.

- Add resolver fixtures for valid, missing, and ambiguous matches.
- Add behavioral tests for the replacement.
- Add a reviewed row to `unchained-macros/impl/tests/native_detours.tsv`
  for each new production detour.
- Run the boundary guard:

  ```sh
  cargo test --locked -p unchained-macros-impl --test native_detour_boundary
  ```

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
