
# PatternSleuth Resolvers

This crate provides procedural macros for automated and manual pattern scanning.

## 1. Standard Resolver (`#[resolver]`)

The standard resolver is used for automated scanning. It generates a struct and handles the platform filtering, scanning, and address manipulation automatically.

### Basic Usage
The simplest form scans a single pattern and returns the result.
```rust
#[resolver(Simple)]
#[pattern("48 8B 05 ?? ?? ?? ?? 48 8B D9")]
pub fn GetRenderer() {}

```

### Call Mode

Use `Call` when the pattern matches a `call` instruction. It will automatically resolve the relative offset (RIP4) of the call target.

```rust
#[resolver(Call)]
#[pattern("E8 ?? ?? ?? ?? 48 8B D0")]
pub fn GetSubsystem() {}

```

### Platform-Specific Patterns

You can provide different patterns for different game builds (e.g., Steam vs Epic Games Store).

```rust
#[resolver(Simple)]
#[pattern(STEAM, "48 8D 05 ?? ?? ?? ?? 48 8B D9")]
#[pattern(EGS, "48 8D 05 ?? ?? ?? ?? 48 8B C1")]
pub fn GetUObjectArray() {}

```

### Advanced Modifiers

* `#[offset(0x8)]`: Adds a static offset to the found address.
* `#[read_rip4]`: Reads a 4-byte relative offset at the resolved address (common for `lea` or `mov` instructions).
* `#[validate(0x48)]`: Ensures the byte at the final resolved address matches the expected value.
* `#[optional]`: If resolution fails, it logs a warning instead of an error.

```rust
#[resolver(Simple)]
#[pattern("48 8B 05 ?? ?? ?? ??")]
#[offset(0x3)]
#[read_rip4]
#[validate(0x48)]
pub fn GetGNames() {}

```

---

## 2. Custom Resolver (`#[custom_resolver]`)

The custom resolver is used when you need to perform complex logic that the automated scanner cannot handle (e.g., multiple scans, manual pointer hopping, or complex verification).

### Features

* **Automatic Platform Filtering**: The macro pre-filters the `#[pattern]` attributes and provides an `active_sigs` slice to your function.
* **Attribute Access**: Attributes like `offset`, `should_read_rip4`, and `validate_expected` are injected into your scope.
* **Anyhow Support**: You can use `anyhow::Result` and the `?` operator; the macro handles error conversion.

### Example: Complex UObjectArray Search

```rust
#[custom_resolver]
#[pattern("45 85 C0 0F ")] // Universal
#[pattern(STEAM, "48 8D 05 ?? ?? ?? ?? 48 8B D9")]
#[pattern(EGS, "48 8D 05 ?? ?? ?? ?? 48 8B C1")]
#[read_rip4]
pub fn GetUObjectArrayCustom() -> anyhow::Result<usize> {
    use patternsleuth::scanner::Pattern;
    use patternsleuth::MemoryTrait;

    // 1. Scan the patterns filtered by the macro for the current platform
    let scans = ::patternsleuth::resolvers::futures::future::join_all(
        active_sigs.iter().map(|p| ctx.scan(Pattern::new(p).unwrap()))
    ).await;

    let addr = *scans.iter()
        .flatten()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No pattern matched for UObjectArray"))?;

    // 2. Use the 'should_read_rip4' bool injected by the macro
    let mut result = addr;
    if should_read_rip4 {
        result = ctx.image().memory.rip4(result)?;
    }

    // 3. Perform manual verification
    let verifier = ctx.image().memory.u8(result + 0x10)?;
    if verifier == 0x00 {
        return Err(anyhow::anyhow!("Invalid object array header"));
    }

    ::log::info!("Resolved Custom UObjectArray at 0x{:X}", result);
    Ok(result)
}

```

## Summary of Injected Variables in Custom Resolvers

When using `#[custom_resolver]`, the following variables are automatically available inside your function body:

| Variable | Type | Description |
| --- | --- | --- |
| `ctx` | `&AsyncContext` | The patternsleuth context for scanning and memory access. |
| `active_sigs` | `Vec<&str>` | Patterns filtered for the current platform (Global + STEAM/EGS). |
| `offset` | `usize` | Value from `#[offset(N)]` (defaults to 0). |
| `should_read_rip4` | `bool` | True if `#[read_rip4]` is present. |
| `validate_expected` | `Option<u8>` | Value from `#[validate(N)]`. |

```
