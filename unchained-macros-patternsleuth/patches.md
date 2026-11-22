Register a named patch factory or immutable `Patch` static against a resolver type.

```rust
use unchained_macros_core::{register_patch, Patch};
#[unchained_macros_core::resolver_selection_key(require = [First, Second])]
enum Key { First, Second }
impl unchained_macros_core::ResolverSelector for Key {
    fn current() -> Self { Self::First }
}
unchained_macros_patternsleuth::define_pattern_resolver!(Target, {
    Key::First: ["AA BB"], Key::Second: ["CC DD"],
});

#[register_patch(Target, Key::First)]
pub fn first_patch(resolved_address: u64) -> Patch {
    Patch::owned_bytes(0x4D, resolved_address.to_le_bytes().to_vec())
}

#[register_patch(Target, Key::Second)]
static SECOND_PATCH: Patch = Patch::bytes(0x4D, &[0x90, 0xE9]);

fn main() {
    unchained_macros_core::validate_patch_registrations().unwrap();
}
```

- The item name is the patch ID; names must be unique. Multiple patches may target
  one resolver. Import aliases retain the resolver's original cache key.
  The registration holds the resolver descriptor, including its selection filter,
  rather than looking it up by name.
- Factories are safe `fn(u64) -> Patch` functions receiving the resolved native
  address. Statics require an explicit `: Patch` type and are cloned when used.
- `Patch::bytes(offset, static_bytes)` and `Patch::nop(offset, count)` are const
  constructors. `owned_bytes` accepts dynamically constructed bytes.
- `Patch::write(offset, value)` writes a `Copy` value. Const-capable
  `Patch::write_ref(offset, &STATIC_VALUE)` copies the referenced value, not its address.
  These retain the old `WRITE` operation; no separate address operation exists.
- `.when(condition)` disables a descriptor without changing coverage or scan roots.
  Evaluate runtime conditions inside factories, not static initializers.
- The host implements `PatchWriter` and calls `apply_patches_for::<PatternsleuthResolver, _>`
  with a `ScanContext` and retained `PatchState`.
  Its existing memory-protection backend remains responsible for writes.
  Typed writes copy raw storage without reading potentially uninitialized padding
  as a byte slice.

## Coverage

An optional enum variant limits applicability. Without one, a patch applies to
every selection and therefore requires an unrestricted resolver.

Required coverage is grouped **by resolver**, across all registered patch names.
Only required keys enabled by that resolver need coverage. Conditions do not
provide or remove coverage, and factories are not evaluated during validation.

```rust
# use unchained_macros_core::{register_patch, Patch};
#[unchained_macros_core::resolver_selection_key(require = [First, Second])]
enum Key { First, Second }
impl unchained_macros_core::ResolverSelector for Key {
    fn current() -> Self { Self::First }
}
unchained_macros_patternsleuth::define_pattern_resolver!(Target, {
    Key::First: ["AA BB"], Key::Second: None,
});
#[register_patch(Target, Key::First)]
static FIRST_ONLY: Patch = Patch::nop(0, 2);
fn main() {
    // Second is not required here: the resolver explicitly disables it.
    unchained_macros_core::validate_patch_registrations().unwrap();
}
```

Separate attributes cannot check aggregate inventory coverage at compile time.
Missing coverage and duplicate patch IDs fail validation before scanning or
applying any patches. All applicable addresses are checked before factories run;
writer failures do not roll back earlier writes.

```rust
# use unchained_macros_core::{register_patch, Patch};
#[unchained_macros_core::resolver_selection_key(require = [First, Second])]
enum Key { First, Second }
unchained_macros_patternsleuth::define_pattern_resolver!(Target, ["AA BB"]);
impl unchained_macros_core::ResolverSelector for Key {
    fn current() -> Self { Self::First }
}
#[register_patch(Target, Key::First)]
static FIRST_ONLY: Patch = Patch::nop(0, 2);
fn main() {
    let error = unchained_macros_core::validate_patch_registrations().unwrap_err();
    assert!(error.contains("required selection Second"));
}
```

## Compile-time compatibility

A universal patch cannot target a restricted resolver:

```compile_fail
# fn main() {}
# use unchained_macros_core::{register_patch, Patch};
#[unchained_macros_core::resolver_selection_key]
enum Key { First, Second }
# impl unchained_macros_core::ResolverSelector for Key {
#     fn current() -> Self { Self::First }
# }
unchained_macros_patternsleuth::define_pattern_resolver!(Target, {
    Key::First: ["AA BB"], Key::Second: None,
});
#[register_patch(Target)]
static INVALID: Patch = Patch::bytes(0, &[0x90]);
```

An explicit key cannot be disabled by its resolver:

```compile_fail
# fn main() {}
# use unchained_macros_core::{register_patch, Patch};
#[unchained_macros_core::resolver_selection_key]
enum Key { First, Second }
# impl unchained_macros_core::ResolverSelector for Key {
#     fn current() -> Self { Self::First }
# }
unchained_macros_patternsleuth::define_pattern_resolver!(Target, {
    Key::First: ["AA BB"], Key::Second: None,
});
#[register_patch(Target, Key::Second)]
static INVALID: Patch = Patch::bytes(0, &[0x90]);
```

The resolver must exist:

```compile_fail
# fn main() {}
#[unchained_macros_core::register_patch(MissingResolver)]
static INVALID: unchained_macros_core::Patch = unchained_macros_core::Patch::nop(0, 1);
```

Factory signatures must match:

```compile_fail
# fn main() {}
unchained_macros_patternsleuth::define_pattern_resolver!(Target, ["AA BB"]);
#[unchained_macros_core::register_patch(Target)]
fn invalid(address: u32) -> unchained_macros_core::Patch {
    unchained_macros_core::Patch::nop(address as u64, 1)
}
```
