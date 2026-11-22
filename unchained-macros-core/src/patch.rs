//! Patch descriptions, host memory backends, and registration validation.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::mem::{MaybeUninit, align_of, size_of};
use crate::{ResolverSelection, SelectionKey, SelectorSnapshot};
use crate::registry::{PatchInfo, ErasedPatchRegistration};

trait StaticWriteSource: Sync {
    fn source(&self) -> *const u8;
}
impl<T: Copy + Sync> StaticWriteSource for T {
    fn source(&self) -> *const u8 { (self as *const T).cast() }
}

#[derive(Clone)]
enum WriteStorage {
    Owned(Vec<MaybeUninit<u8>>),
    Static(&'static dyn StaticWriteSource),
}

/// An erased Copy value. Raw copying preserves pointer provenance and padding;
/// backends must not turn potentially uninitialized padding into `&[u8]`.
#[derive(Clone)]
pub struct WriteValue {
    storage: WriteStorage,
    size: usize,
    alignment: usize,
}

impl std::fmt::Debug for WriteValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteValue").field("size", &self.size).field("alignment", &self.alignment).finish()
    }
}

impl WriteValue {
    fn owned<T: Copy + 'static>(value: T) -> Self {
        let mut storage = Vec::<MaybeUninit<u8>>::with_capacity(size_of::<T>());
        unsafe {
            std::ptr::copy_nonoverlapping(
                (&value as *const T).cast::<MaybeUninit<u8>>(), storage.as_mut_ptr(), size_of::<T>(),
            );
            storage.set_len(size_of::<T>());
        }
        Self { storage: WriteStorage::Owned(storage), size: size_of::<T>(), alignment: align_of::<T>() }
    }

    pub fn as_ptr(&self) -> *const u8 {
        match &self.storage {
            WriteStorage::Owned(storage) => storage.as_ptr().cast(),
            WriteStorage::Static(source) => StaticWriteSource::source(*source),
        }
    }
    pub fn size(&self) -> usize { self.size }
    pub fn alignment(&self) -> usize { self.alignment }
}

#[derive(Debug, Clone)]
enum Operation {
    Bytes(Cow<'static, [u8]>),
    Nop(usize),
    Write(WriteValue),
}

/// A relative memory edit. Factories receive the resolved native address.
#[derive(Debug, Clone)]
pub struct Patch {
    offset: u64,
    operation: Operation,
    enabled: bool,
}

impl Patch {
    pub const fn bytes(offset: u64, bytes: &'static [u8]) -> Self {
        Self { offset, operation: Operation::Bytes(Cow::Borrowed(bytes)), enabled: true }
    }

    pub fn owned_bytes(offset: u64, bytes: Vec<u8>) -> Self {
        Self { offset, operation: Operation::Bytes(Cow::Owned(bytes)), enabled: true }
    }

    pub const fn nop(offset: u64, count: usize) -> Self {
        Self { offset, operation: Operation::Nop(count), enabled: true }
    }

    /// Write a Copy value without interpreting its representation as initialized bytes.
    pub fn write<T: Copy + 'static>(offset: u64, value: T) -> Self {
        Self { offset, operation: Operation::Write(WriteValue::owned(value)), enabled: true }
    }

    /// Const form for statics. Copies the referenced value, not its address.
    pub const fn write_ref<T: Copy + Sync + 'static>(offset: u64, value: &'static T) -> Self {
        Self {
            offset,
            operation: Operation::Write(WriteValue {
                storage: WriteStorage::Static(value), size: size_of::<T>(), alignment: align_of::<T>(),
            }),
            enabled: true,
        }
    }

    pub const fn when(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Apply through the host's memory-protection/write backend.
    ///
    /// # Safety
    /// The enabled patch's target range must be valid for its native contract.
    pub unsafe fn apply<W: PatchWriter>(&self, resolved_address: u64, writer: &mut W) -> Result<bool, String> {
        if !self.enabled { return Ok(false); }
        let address = resolved_address.checked_add(self.offset)
            .ok_or_else(|| "Patch address overflow".to_string())?;
        match &self.operation {
            Operation::Bytes(bytes) => unsafe { writer.bytes(address, bytes)?; },
            Operation::Nop(count) => unsafe { writer.nop(address, *count)?; },
            Operation::Write(value) => {
                if address % value.alignment() as u64 != 0 {
                    return Err("Unaligned typed patch address".into());
                }
                unsafe { writer.write(address, value)?; }
            }
        }
        Ok(true)
    }
}

/// Host-owned memory protection and write operations.
pub trait PatchWriter {
    /// # Safety
    /// The caller must supply a valid native target range.
    unsafe fn bytes(&mut self, address: u64, bytes: &[u8]) -> Result<(), String>;
    /// # Safety
    /// The caller must supply a valid native instruction range.
    unsafe fn nop(&mut self, address: u64, count: usize) -> Result<(), String>;
    /// # Safety
    /// The destination must be valid, nonoverlapping, and aligned for this value.
    /// Copy raw storage; its padding need not be initialized.
    unsafe fn write(&mut self, address: u64, value: &WriteValue) -> Result<(), String>;
}

#[derive(Debug, PartialEq, Eq)]
pub struct PatchApplication {
    pub name: &'static str,
    pub resolver: &'static str,
    pub applied: bool,
}

#[doc(hidden)]
pub fn validate_patch_coverage<R: ResolverSelection<E>, E: SelectionKey>(
    _: E, patches: &[&dyn PatchInfo],
) -> Result<(), String> {
    for key in E::REQUIRED {
        if crate::__private::dictionary_allows(key, R::SELECTIONS)
            && !patches.iter().any(|patch| patch.applies_to(key))
        {
            return Err(format!("Patches for resolver {} do not cover required selection {}", R::NAME, key.name()));
        }
    }
    Ok(())
}

fn validate(patches: &[&dyn PatchInfo]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for patch in patches {
        if !names.insert(patch.name()) {
            return Err(format!("Duplicate patch name {}", patch.name()));
        }
        groups.entry((patch.resolver().backend(), patch.resolver().name())).or_default().push(*patch);
    }
    for group in groups.values() {
        if group.iter().any(|patch| patch.resolver().identity() != group[0].resolver().identity()) {
            return Err(format!("Patches reference different descriptors for resolver {}", group[0].resolver().name()));
        }
        for patch in group {
            patch.validate_group(group)?;
        }
    }
    Ok(())
}

/// Validate unique patch names and required coverage across each resolver's group.
/// This does not run factories, conditions, or memory writes.
pub fn validate_patch_registrations() -> Result<(), String> {
    validate(&inventory::iter::<ErasedPatchRegistration>.into_iter().map(|entry| entry.info).collect::<Vec<_>>())
}

/// Validate and resolve every applicable patch before invoking factories/writers.
/// Factories must describe edits, not modify native memory themselves.
///
/// # Safety
/// Each resolved address and resulting edit must satisfy its native contract.
/// Earlier writes are not rolled back if a later writer call fails.
/// Host-owned state prevents successful edits from running again during retries.
#[derive(Default)]
pub struct PatchState { completed: BTreeSet<&'static str> }

/// # Safety
/// Each resolved address and edit must satisfy its native contract.
/// Keep one state for the initialization session; serialize calls with that state.
pub unsafe fn apply_patches_for<R: crate::AddressResolver, W: PatchWriter>(
    context: &R::Context, snapshot: &SelectorSnapshot, writer: &mut W,
    state: &mut PatchState,
) -> Result<Vec<PatchApplication>, String> {
    snapshot.validate_registered()?;
    validate_patch_registrations()?;
    let selected = crate::patch_inventory_for::<R>().filter(|patch| snapshot.supports_patch(*patch)
        && !state.completed.contains(patch.name))
        .map(|patch| {
            if !patch.resolver.supports(snapshot) {
                return Err(format!("Patch {} targets unavailable resolver {}", patch.name, patch.resolver.name));
            }
            let address = unsafe { patch.resolver.implementation.resolve(context) }
                .map_err(|error| format!("No address for patch {} resolver {}: {error}", patch.name, patch.resolver.name))?.get() as u64;
            Ok((patch, address))
        }).collect::<Result<Vec<_>, String>>()?;
    selected.into_iter().map(|(patch, address)| {
        let applied = unsafe { (patch.factory)(address).apply(address, writer) }
            .map_err(|error| format!("Patch {}: {error}", patch.name))?;
        state.completed.insert(patch.name);
        Ok(PatchApplication { name: patch.name, resolver: patch.resolver.name, applied })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResolverDeclaration;
    struct Mock;
    impl crate::AddressResolver for Mock {
        type Context = ();
        unsafe fn resolve(&self, _: &()) -> Result<std::num::NonZeroUsize, String> { panic!("validation must not resolve") }
    }
    type PatchRegistration = crate::PatchRegistration<Mock>;
    type ResolverDescriptor = crate::Resolver<Mock>;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Key { First, Second }
    impl SelectionKey for Key {
        const REQUIRED: &'static [Self] = &[Self::First, Self::Second];
        fn name(self) -> &'static str { match self { Self::First => "First", Self::Second => "Second" } }
    }

    struct Both;
    impl ResolverDeclaration for Both {
        type Backend = Mock;
        const NAME: &'static str = "Both";
        const UNRESTRICTED: bool = true;
        const DESCRIPTOR: &'static ResolverDescriptor = &BOTH;
    }
    impl ResolverSelection<Key> for Both {
        const SELECTIONS: &'static [(Key, bool)] = &[];
    }
    struct FirstOnly;
    impl ResolverDeclaration for FirstOnly {
        type Backend = Mock;
        const NAME: &'static str = "FirstOnly";
        const UNRESTRICTED: bool = false;
        const DESCRIPTOR: &'static ResolverDescriptor = &FIRST_DESCRIPTOR;
    }
    impl ResolverSelection<Key> for FirstOnly {
        const SELECTIONS: &'static [(Key, bool)] = &[(Key::First, true), (Key::Second, false)];
    }

    static BOTH: ResolverDescriptor = ResolverDescriptor { name: "Both", implementation: Mock, filter: None };
    static FIRST_DESCRIPTOR: ResolverDescriptor = ResolverDescriptor { name: "FirstOnly", implementation: Mock, filter: None };
    static OTHER: ResolverDescriptor = ResolverDescriptor { name: "Other", implementation: Mock, filter: None };
    static NAMESAKE: ResolverDescriptor = ResolverDescriptor { name: "Both", implementation: Mock, filter: None };
    fn never_construct(_: u64) -> Patch { panic!("validation must not construct patches") }
    fn first(key: &dyn std::any::Any) -> bool { crate::__private::selection_matches(key, Key::First) }
    fn second(key: &dyn std::any::Any) -> bool { crate::__private::selection_matches(key, Key::Second) }
    fn both(group: &[&dyn PatchInfo]) -> Result<(), String> {
        validate_patch_coverage::<Both, _>(Key::First, group)
    }
    static FIRST: PatchRegistration = PatchRegistration {
        name: "first", resolver: &BOTH, factory: never_construct, applies_to: first, validate_group: both,
        selector: Some(std::any::TypeId::of::<Key>),
    };
    static SECOND: PatchRegistration = PatchRegistration {
        name: "second", resolver: &BOTH, factory: never_construct, applies_to: second, validate_group: both,
        selector: Some(std::any::TypeId::of::<Key>),
    };
    static FIRST_ONLY: PatchRegistration = PatchRegistration {
        name: "only_first", resolver: &FIRST_DESCRIPTOR, factory: never_construct, applies_to: first,
        selector: Some(std::any::TypeId::of::<Key>),
        validate_group: |group| validate_patch_coverage::<FirstOnly, _>(Key::First, group),
    };
    static DUPLICATE: PatchRegistration = PatchRegistration {
        name: "first", resolver: &BOTH, factory: never_construct, applies_to: second, validate_group: both,
        selector: Some(std::any::TypeId::of::<Key>),
    };
    static OTHER_RESOLVER: PatchRegistration = PatchRegistration {
        name: "other", resolver: &OTHER, factory: never_construct, applies_to: second, validate_group: both,
        selector: Some(std::any::TypeId::of::<Key>),
    };
    static CONFLICTING_DESCRIPTOR: PatchRegistration = PatchRegistration {
        name: "namesake", resolver: &NAMESAKE, factory: never_construct, applies_to: second,
        selector: Some(std::any::TypeId::of::<Key>), validate_group: both,
    };

    #[test_case::test_case(&[&FIRST], Some("required selection Second"); "missing enabled selection")]
    #[test_case::test_case(&[&FIRST, &SECOND], None; "different patch names jointly cover resolver")]
    #[test_case::test_case(&[&FIRST_ONLY], None; "resolver disables second selection")]
    #[test_case::test_case(&[&FIRST, &DUPLICATE], Some("Duplicate patch name first"); "duplicate names")]
    #[test_case::test_case(&[&FIRST, &OTHER_RESOLVER], Some("required selection Second"); "other resolver does not supply coverage")]
    #[test_case::test_case(&[&FIRST, &CONFLICTING_DESCRIPTOR], Some("different descriptors"); "namesake descriptor does not supply coverage")]
    fn coverage_is_grouped_by_resolver(patches: &[&PatchRegistration], expected: Option<&str>) {
        let patches: Vec<&dyn PatchInfo> = patches.iter().map(|patch| *patch as &dyn PatchInfo).collect();
        match expected {
            Some(message) => assert!(validate(&patches).unwrap_err().contains(message)),
            None => validate(&patches).unwrap(),
        }
    }
}
