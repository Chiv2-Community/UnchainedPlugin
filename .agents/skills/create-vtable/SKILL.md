---
name: create-vtable
description: Create or extend a native vtable declaration under src/Chivalry2 using rust_ctables. Use when modeling virtual methods, verifying slot offsets and receivers, or extending inherited primary vtables.
---

# Creating a vtable

Declare a verified Rust view of a native vtable and its virtual methods.

- Follow [Declare vtable methods](../../../CONTRIBUTING.md#declare-vtable-methods),
  [Inherited vtables](../../../CONTRIBUTING.md#inherited-vtables), and the
  [native source layout](../../../docs/engine-sources.md).
- Use the project's pinned `rust_ctables` API and nearby native declarations.
- This skill models virtual dispatch; it does not replace a live game's vtable.

## Verify the native table

Establish the object layout, table, slots, and ABI in the latest target binary.

- Follow the [resolver skill's binary-analysis workflow](../create-resolver/SKILL.md)
  to identify the current binary and use available analysis tools.
- A vtable call normally needs no pattern resolver. Do not create one merely
  to declare slots on an existing native object.
- Use source, matching symbols, constructors, vptr writes, and virtual call sites
  to identify the table and owning type.
- Verify the vptr's position within the object or base subobject.
- Confirm each slot's target, receiver, parameters, return convention, and any
  hidden return-storage arguments.
- Distinguish implementations, adjustor thunks, destructors, and unrelated tables.
- Do not assume a base and derived class use the same implementation.
- Check supported storefront/build variants; report missing evidence explicitly.
- Do not infer Rust references, object size, or ownership from a decompiler alone.
- Do not modify running objects or tables as part of discovery.

## Declare the layout

Keep native types and virtual methods under their verified source owner.

- Place declarations under `src/Chivalry2/*`; add new modules to `mod.rs`.
- Reuse existing native types rather than introducing duplicate layouts.
- Apply `#[rust_ctables::cpp_struct]` to the containing struct.
- Put `#[vtable(...)]` on the vptr field or inherited base field.
- `offset(...)` is a byte offset within the table, not an RVA or slot number.
- Windows x64 slots are eight bytes; method declarations advance one slot each.
- Skip unmodeled entries with explicit offsets; do not fabricate intervening methods.
- Match C++ method spelling; keep ABI/safety notes rather than duplicate headers.
- Name native destructor entries `Destructor`, including deleting-destructor
  thunks; preserve their flags, return type, ownership rules, and slot.
- Preserve native acronyms: `TBLFoo`, never `TblFoo`.
- Match receiver mutability and native ABI. Raw mutable receivers use
  `self: *mut Self` when reference guarantees are unavailable.
- Keep declarations separate from plugin policy, logging hooks, and patches.

Existing `FUniqueNetId` pattern; extend the actual declaration rather than
creating another copy:

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
```

- `IsValid` uses slot 5 (`0x28`); `ToString` uses slot 6 (`0x30`).
- The string method explicitly receives caller-provided storage.
- See [FOutputDevice](../../../src/Chivalry2/Engine/Runtime/Core/Misc/OutputDevice.rs)
  for raw receivers and
  [FInternetAddr](../../../src/Chivalry2/Engine/Runtime/Sockets/IPAddress.rs)
  for a sparse table declaration.

## Extend an inherited table

Use the embedded base's primary vptr rather than adding a second one.

- Apply `#[inherit]` and `#[vtable(...)]` to the verified base field.
- Keep slot offsets relative to the vtable start.
- Verify base-subobject placement and receiver adjustment.
- Do not guess multiple-inheritance layouts or overlap already bound slots.

Existing `UObject::GetWorld` pattern:

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

## Call and intercept safely

Generated virtual methods are unsafe calls through the current object's table.

- Require a live object/table and valid arguments; obey threading and aliasing rules.
- Do not construct a Rust-owned native object merely to call its vtable.
- Preserve native ownership and return-buffer lifetimes.
- Virtual dispatch does not guarantee original-call bypass and may enter a detour.
- Generated vtable methods are not `bind_func_detour` targets.
- To intercept a concrete implementation, follow the
  [runtime binding skill](../create-runtime-binding/SKILL.md), then the
  [detour skill](../create-detour/SKILL.md). Resolve the implementation's entry,
  declare a separate binding, and call that binding for the original.

## Validate

Verify native slots and layout from binary evidence; compile the declaration.

- Compile on the supported Windows target with affected features on and off.
- Verify object/base offsets, slot targets, and inherited receiver adjustments
  against the identified binary.
- Do not add tests asserting native layouts, slot offsets, signatures, pointer
  forwarding, or binding metadata, including synthetic vtables and typed
  function-pointer assignments.
- Test actual Rust helper or plugin behavior where applicable. Keep generic
  vtable-code-generation tests separate from production native declarations.
- Do not recreate native-binding boundary guards or baseline tables.
- Report binary identity, source/module path, slot byte offsets, ABI evidence,
  tests, and unsupported variants.
