# Contributing to Unchained Plugin

This guide covers game hooks, commands, vote types, and event subscribers.

## Creating game hooks

A hook has three parts:

1. A **resolver** finds a native address.
2. A **binding** in `src/chivalry2/*` declares how to call that address.
3. A **detour** in `src/unchained/patches/*` implements plugin behavior.

Keep resolvers and bindings with native types. Group detours by plugin purpose,
not by engine class. Do not use the legacy `CREATE_HOOK!` or
`bind_resolved_func_detour` APIs for new production hooks.

### 1. Find the native owner and ABI

- Use native source, debug symbols, and disassembly to establish the owner,
  parameter types/order, return type, and calling convention.
- Follow the [native source layout](docs/engine-sources.md): keep `runtime` and
  `third_party`; omit `Public`, `Private`, and `Classes`. Game-plugin bindings
  belong under `src/chivalry2/plugins/`.
- Add new files to their parent `mod.rs`. Keep resolver declarations before
  native types and bindings.
- Do not infer a receiver from a function's name. Static methods have no
  receiver. Represent hidden return-storage arguments explicitly.
- Prefer raw pointers unless native validity, lifetime, and aliasing guarantees
  justify Rust references. Do not invent layouts for opaque types.

### 2. Create a resolver

Reuse an existing resolver when it already identifies the target. Otherwise,
declare one beside the native binding.

#### Byte-pattern resolvers

`define_pattern_resolver!` scans the process image. `??` matches any byte;
wildcard relocations and build-dependent displacements rather than copying
absolute addresses.

This existing resolver finds the static UDP-ping function:

```rust
define_pattern_resolver!(UDPEcho, [
    "48 89 5C 24 20 55 56 57 48 81 EC 90 00 00 00 49 8B D8 0F 29 B4 24 80 00 00 00 48 8B E9"
]);
```

Use the following scan modes:

| Mode | Result |
| --- | --- |
| `Simple` (default) | One unique matched address across the supplied patterns; missing or conflicting results fail. |
| `Call` | Decode a four-byte relative displacement and resolve its target. Put `\|` at the displacement, e.g. `"E8 \| ?? ?? ?? ??"`. |
| `First` | First match across the supplied patterns; does not reject ambiguity. |
| `XrefFirst` / `XrefLast` | Scan the first supplied string pattern, find references, then select the first/last containing root function. |

Function bindings must resolve to function entries, not arbitrary instructions
inside them. Use `First` or xref ordering only when that selection is verified
for the supported builds.

Specify a mode as the second argument:

```rust
define_pattern_resolver!(ExampleCallTarget, Call, [
    "E8 | ?? ?? ?? ?? 4C 39 ?8 74 3?"
]);
```

For a string-reference search, supply a string pattern rather than hex bytes:

```rust
define_pattern_resolver!(ExampleStringOwner, XrefFirst, [
    patternsleuth::resolvers::unreal::util::utf8_pattern("Unique native diagnostic")
]);
```

This and the examples below illustrate syntax; replace example patterns with
verified signatures. Do not add duplicate declarations for existing resolvers.

For different storefront builds, supply platform-specific patterns:

```rust
define_pattern_resolver!(ExampleFunction, {
    STEAM: ["48 89 5C 24 ?? 57"],
    EGS: ["40 53 48 83 EC ??"],
    OTHER: ["48 8B C4 48 89 58 ??"]
});
```

The macro combines the current platform's patterns with `OTHER`; `OTHER` is
shared coverage, not a fallback used only after other patterns fail.
Platform selection alone does not restrict hook or patch installation.
For a target that exists only on one storefront, use:

```rust
define_single_platform_pattern_resolver!(STEAM, ExampleSteamOnly, [
    "48 89 5C 24 ?? 57"
]);
```

This registers the restriction for the resolver and its hooks/patches.
Supported platform names are `STEAM`, `EGS`, `XBOX`, and `OTHER`.

#### Custom resolvers

Use `define_custom_resolver!` with BinaryBloodhound for composed searches,
such as matching an instruction inside a function and recovering its entry:

```rust
use binary_bloodhound::BinaryMatcher;

define_custom_resolver!(ExampleRootFunction, {
    BinaryMatcher::patterns(["48 89 5C 24 ?? 57"])?
        .root_functions()
        .addresses()
        .ensure_one()
});
```

The body must produce a no-input matcher yielding exactly one `u64` address.
Use `|platform| { ... }` instead of `{ ... }` when matcher construction depends
on the storefront. This selects the search; it does not register a platform
restriction. Construction and resolution errors retain the resolver name.

For direct patternsleuth logic, `define_pattern_resolver!` also accepts an
async scan body returning `Result<u64, ResolveError>`:

```rust
define_pattern_resolver!(ExampleCustomScan, ["48 89 5C 24 ?? 57"], |ctx, patterns| {
    let pattern = patternsleuth::scanner::Pattern::new(patterns[0]).unwrap();
    let matches = ctx.scan(pattern).await;
    patternsleuth::resolvers::ensure_one(matches)
});
```

Existing mixed-mode resolvers can compose `Simple_signature(...)`,
`Call_signature(...)`, `First_signature(...)`, `XrefFirst_signature(...)`, and
`XrefLast_signature(...)` inside a pattern list. Import these helpers from
`unchained_macros::signature`; each selected helper resolves its own pattern,
then `MultiCall` requires one unique resulting address.

Prefer a specific resolver over first-match guesses. Verify the result against
each supported game build, including no-match and ambiguous-match cases.
See [resolver implementation](unchained-macros/src/signatures.rs) and
[custom resolver examples](unchained-macros/README.md#custom-resolvers).

### 3. Declare a native binding

Add an empty `unsafe fn` inside `#[runtime_bindings] impl Type`. Its resolver
argument is a type path; use the declared resolver name, not a renamed import.
Set visibility so the plugin detour can access it.

For example, the existing static UDP-ping binding is:

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

The callback type is opaque; this binding only passes its pointer. For an instance
method, use an explicit receiver such as `self: *mut Self`. The detour must
declare the corresponding first argument, such as `this: *mut NativeType`.
The macros generate Windows x64 `extern "C"` calls; they do not infer C++ ABI
details or validate the declaration against the binary.

Other binding sources:

- **Existing patternsleuth resolver:** pass its full type path to
  `bind_resolved_func`, e.g.
  `patternsleuth::resolvers::unreal::game_loop::UGameEngineTick`.
- **Request offset registry:** use
  `#[unchained_macros::bind_resolved_request_function(GetPost_GetMotd)]`
  under `runtime_bindings`. The producer must populate that exact key in
  `OffsetRegisty` with an RVA, not an absolute address. See the existing
  [OpenAPI bindings and registry](src/chivalry2/plugins/online_subsystem_playfab/open_api_merged/open_api_client_api.rs).
- **Native globals:** use `bind_resolver_value(Resolver, Address)` for an inline
  object's address or `bind_resolver_value(Resolver, Value)` to read a `Copy`
  value from a resolved slot. These are value accessors, not detour targets.
  See [required native values](unchained-macros/README.md#required-native-values).

Bindings are registered automatically. Startup resolves addresses and initializes
bindings before attaching detours. Missing function bindings fail initialization;
calling an uninitialized binding panics.

### 4. Create the detour

Place the replacement in the relevant module under `src/unchained/patches/`.
Put logging-only hooks in `diagnostics.rs`. Add new modules to `mod.rs`, and put
all detour functions after imports, before patches, helpers, state, and tests.

Target the **binding method**, not the resolver or Rust wrapper address:

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

This is an alternative replacement, not an additional hook for the existing
UDP target: only one detour may own a native address.

- The macro checks the replacement against the binding's signature at compile
  time, including receiver, return type, and lifetime relationships.
- The optional condition defaults to `ACTIVE`. A condition is evaluated during
  attachment; activation also requires global auto-activation. It is not checked
  on every invocation.
- Calling the native binding invokes the original implementation through the
  trampoline, even from inside its detour. It does not recursively invoke the
  replacement. A disabled but attached detour still supplies its trampoline.
- Preserve native thread, pointer, ownership, and callback-lifetime requirements.
  Do not let Rust panics unwind through native callers.
- Keep feature gates on the binding and replacement consistent. Prefer a
  module-level gate when a whole plugin module needs the same feature.

For instruction patches rather than replacements, keep `CREATE_PATCH!` under
`src/unchained/patches/`; its resolver stays in the native model.
See [binding and trampoline contracts](docs/native-bindings.md) for details.

### 5. Validate

- Add resolver fixtures and behavioral tests. A typed function-pointer assignment
  is not a behavioral test; the detour macro already checks signature agreement.
- For a new production detour, add a reviewed row to
  `unchained-macros/impl/tests/native_detours.tsv`. The boundary guard checks
  native signatures, resolver keys, conditions, feature gates, and module placement.
- Run `cargo test -p unchained-macros-impl --test native_detour_boundary`.
- Run `cargo test --locked --lib` on the supported Windows target, with the
  relevant optional features both enabled and disabled. Linux cross-builds use
  [cargo-xwin](README.md#building-the-windows-dll-on-linux); running Windows tests
  there also requires a runner such as Wine.
- Validate resolution and behavior in the supported game builds. Unit tests
  cannot prove that a shipping address has the ABI declared in Rust.

## Creating a New Command

The command system uses a trait-based approach integrated with `clap` for argument parsing.

### 1. Define Command Arguments
Create a struct for your command's arguments and derive `Parser` and `CommandFactory`.

```rust
use clap::{Parser, CommandFactory};

#[derive(Parser, CommandFactory, Clone, Debug)]
#[command(name = "mycommand", about = "Description of what my command does")]
pub struct MyCommandArgs {
    #[arg(help = "A required argument")]
    pub target: String,
}
```

### 2. Implement the `Command` Trait
Implement the `Command<MyCommandArgs>` trait for your command struct.

```rust
use async_trait::async_trait;
use crate::commands::{Command, CommandResult};
use crate::events::models::{CommandRequest, PermissionFlags};

pub struct MyCommand;

#[async_trait]
impl Command<MyCommandArgs> for MyCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::all() // Or specific flags
    }

    async fn execute(&self, args: MyCommandArgs, req: &CommandRequest) {
        // Your logic here
        println!("Executing mycommand with target: {}", args.target);
    }
}
```

### 3. Register the Command
Commands must be registered to the `CommandSubscriber`.

- **Always Active**: Register in `src/features/events.rs` within `initialize_subscribers`.
- **Discord Only**: Register in `src/features/discord.rs` (if applicable).

Example in `src/features/events.rs`:
```rust
command_subscriber.register(MyCommand);
```

---

## Creating a New Vote Type

The voting system is similar to the command system but uses the `VoteType` trait.

### 1. Define Vote Arguments
Define a struct for the vote's arguments using `clap`.

```rust
#[derive(Parser, CommandFactory, Clone, Debug)]
#[command(name = "myvote")]
pub struct MyVoteArgs {
    pub value: i32,
}
```

### 2. Implement the `VoteType` Trait
Implement `VoteType<MyVoteArgs>` for your vote logic.

```rust
use crate::modules::vote::VoteType;

pub struct MyVote;

impl VoteType<MyVoteArgs> for MyVote {
    fn title(&self) -> String { "My Vote".to_string() }
    fn description(&self) -> String { "Vote for something".to_string() }
    fn vote_description(&self, args: MyVoteArgs) -> String {
        format!("Should we set the value to {}?", args.value)
    }
    fn min_yes_vote_ratio(&self) -> f32 { 0.5 }
    fn min_votes_required_ratio(&self) -> f32 { 0.2 }
    fn on_success(&self, args: MyVoteArgs) {
        // Success logic
    }
    fn clone_box(&self) -> Box<dyn ErasedVoteType> {
        Box::new(VoteTypeHandler::new(self.clone()))
    }
}
```

### 3. Register the Vote Type
Register your vote type to the `VoteCommand` in `src/features/events.rs` within `initalize_vote_commands`.

```rust
vote_command.register(MyVote).await;
```

---

## Creating a New Subscriber

Subscribers listen for events on an `EventBus`.

### 1. Implement the `Subscriber` Trait
Implement `Subscriber<T>` for the event type you are interested in (e.g., `GameEvent` or `BroadcastMessage`).

```rust
use async_trait::async_trait;
use crate::events::bus::Subscriber;

pub struct MySubscriber;

#[async_trait]
impl Subscriber<GameEvent> for MySubscriber {
    fn identifier(&self) -> &'static str { "MySubscriber" }
    
    async fn on_event(&mut self, event: &GameEvent) {
        // Handle the event
    }

    async fn on_tick(&mut self) {
        // Optional: periodic logic
    }
}
```

### 2. Register the Subscriber
Register the subscriber to the appropriate bus in `src/features/events.rs`.

```rust
let _ = game_event_bus.subscribe(Box::new(MySubscriber)).await;
```

## Architecture and State Management

### Global Statics vs. Dependency Injection

This project operates by injecting a DLL into Chivalry 2. Because we do not have full control over the game's methods or their lifecycle, some **globally accessible mutable static values** are necessary to share state between DLL hooks.

However, **you should avoid adding new global statics** whenever possible. 

- **Global Statics**: Should only be used if the state *must* be accessible inside a low-level DLL Hook.
- **Dependency Injection**: For all other components (Commands, Votes, Subscribers), you should inject the dependencies they need (such as event publishers or shared state) into their structs during initialization.

### Example: Injecting Dependencies

When creating a new component, pass its dependencies through its constructor:

```rust
pub struct MySubscriber {
    broadcaster: &'static EventPublisher<BroadcastMessage>,
    shared_data: Arc<Mutex<MyData>>,
}

impl MySubscriber {
    pub fn new(
        broadcaster: &'static EventPublisher<BroadcastMessage>, 
        shared_data: Arc<Mutex<MyData>>
    ) -> Self {
        Self { broadcaster, shared_data }
    }
}
```

Then, initialize and register it in `src/features/events.rs`:

```rust
let my_data = Arc::new(Mutex::new(MyData::new()));
let my_subscriber = MySubscriber::new(broadcast_message_publisher, my_data);
let _ = game_event_bus.subscribe(Box::new(my_subscriber)).await;
```

---

## Logging

See [log locations](docs/logging.md#log-location) and
[chat logging](docs/logging.md#chat-logging) for paths, formats, and chat-only encoders.

### Emitting diagnostics

Use the shared logging macros.
See [emitting diagnostics](docs/logging.md#contributor-guide-emitting-diagnostics)
for macro imports and function metadata.

Use `logging::log4rs::register` when adding pattern extensions to another log4rs
deserializer registry, or use `logging::log4rs::pattern::PatternEncoder` directly.
The logging module is declared with `#[macro_use]` before callers; its internal
macros are declared first too.

### Configuring output

See the logging guide for [defaults and configuration selection](docs/logging.md#defaults),
[structured logging](docs/logging.md#structured-logging),
[patterns and function fields](docs/logging.md#format-log-lines), and
[target levels](docs/logging.md#set-log-levels).
