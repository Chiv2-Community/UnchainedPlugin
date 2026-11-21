Register a named patch factory or immutable `Patch` static against a resolver type.

```rust
use unchained_macros::{register_patch, Patch};
#[unchained_macros::resolver_selection_key(require = [First, Second])]
enum Key { First, Second }
unchained_macros::define_pattern_resolver!(Target, select Key; [Key::First], {
    Key::First: ["AA BB"], Key::Second: ["CC DD"],
});

#[register_patch(Target, Key::First)]
pub fn first_patch(resolved_address: u64) -> Patch {
    Patch::owned_bytes(0x4D, resolved_address.to_le_bytes().to_vec())
}

#[register_patch(Target, Key::Second)]
static SECOND_PATCH: Patch = Patch::bytes(0x4D, &[0x90, 0xE9]);

fn main() {
    unchained_macros::validate_patch_registrations().unwrap();
}
```

- The item name is the patch ID; names must be unique. Multiple patches may target
  one resolver. Import aliases retain the resolver's original cache key.
- Factories are safe `fn(u64) -> Patch` functions receiving the resolved native
  address. Statics require an explicit `: Patch` type and are cloned when used.
- `Patch::bytes(offset, static_bytes)` and `Patch::nop(offset, count)` are const
  constructors. `owned_bytes` accepts dynamically constructed bytes.
- `.when(condition)` disables a descriptor without changing coverage or scan roots.
  Evaluate runtime conditions inside factories, not static initializers.
- The host implements `PatchWriter` and calls `apply_registered_patches`.
  Its existing memory-protection backend remains responsible for writes.

## Coverage

An optional enum variant limits applicability. Without one, a patch applies to
every selection and therefore requires an unrestricted resolver.

Required coverage is grouped **by resolver**, across all registered patch names.
Only required keys enabled by that resolver need coverage. Conditions do not
provide or remove coverage, and factories are not evaluated during validation.

```rust
# use unchained_macros::{register_patch, Patch};
#[unchained_macros::resolver_selection_key(require = [First, Second])]
enum Key { First, Second }
unchained_macros::define_pattern_resolver!(Target, select Key; [Key::First], {
    Key::First: ["AA BB"], Key::Second: None,
});
#[register_patch(Target, Key::First)]
static FIRST_ONLY: Patch = Patch::nop(0, 2);
fn main() {
    // Second is not required here: the resolver explicitly disables it.
    unchained_macros::validate_patch_registrations().unwrap();
}
```

Separate attributes cannot check aggregate inventory coverage at compile time.
Missing coverage and duplicate patch IDs fail validation before scanning or
applying any patches. All applicable addresses are checked before factories run;
writer failures do not roll back earlier writes.

```rust
# use unchained_macros::{register_patch, Patch};
#[unchained_macros::resolver_selection_key(require = [First, Second])]
enum Key { First, Second }
unchained_macros::define_pattern_resolver!(Target, ["AA BB"]);
#[register_patch(Target, Key::First)]
static FIRST_ONLY: Patch = Patch::nop(0, 2);
fn main() {
    let error = unchained_macros::validate_patch_registrations().unwrap_err();
    assert!(error.contains("required selection Second"));
}
```

## Compile-time compatibility

A universal patch cannot target a restricted resolver:

```compile_fail
# fn main() {}
# use unchained_macros::{register_patch, Patch};
#[unchained_macros::resolver_selection_key]
enum Key { First, Second }
unchained_macros::define_pattern_resolver!(Target, select Key; [Key::First], {
    Key::First: ["AA BB"], Key::Second: None,
});
#[register_patch(Target)]
static INVALID: Patch = Patch::bytes(0, &[0x90]);
```

An explicit key cannot be disabled by its resolver:

```compile_fail
# fn main() {}
# use unchained_macros::{register_patch, Patch};
#[unchained_macros::resolver_selection_key]
enum Key { First, Second }
unchained_macros::define_pattern_resolver!(Target, select Key; [Key::First], {
    Key::First: ["AA BB"], Key::Second: None,
});
#[register_patch(Target, Key::Second)]
static INVALID: Patch = Patch::bytes(0, &[0x90]);
```

The resolver must exist:

```compile_fail
# fn main() {}
#[unchained_macros::register_patch(MissingResolver)]
static INVALID: unchained_macros::Patch = unchained_macros::Patch::nop(0, 1);
```

Factory signatures must match:

```compile_fail
# fn main() {}
unchained_macros::define_pattern_resolver!(Target, ["AA BB"]);
#[unchained_macros::register_patch(Target)]
fn invalid(address: u32) -> unchained_macros::Patch {
    unchained_macros::Patch::nop(address as u64, 1)
}
```
