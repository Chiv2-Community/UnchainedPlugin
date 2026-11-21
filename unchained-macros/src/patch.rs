//! Patch descriptions, host memory backends, and registration validation.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use crate::{PatchRegistration, ResolverSelection, SelectionKey};

#[derive(Debug, Clone)]
enum Operation {
    Bytes(Cow<'static, [u8]>),
    Nop(usize),
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
}

#[derive(Debug, PartialEq, Eq)]
pub struct PatchApplication {
    pub name: &'static str,
    pub resolver: &'static str,
    pub applied: bool,
}

#[doc(hidden)]
pub fn validate_patch_coverage<R: ResolverSelection<E>, E: SelectionKey>(
    _: E, patches: &[&PatchRegistration],
) -> Result<(), String> {
    for key in E::REQUIRED {
        if crate::__private::dictionary_allows(key, R::SELECTIONS)
            && !patches.iter().any(|patch| (patch.applies_to)(key))
        {
            return Err(format!("Patches for resolver {} do not cover required selection {}", R::NAME, key.name()));
        }
    }
    Ok(())
}

fn validate(patches: &[&PatchRegistration]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for patch in patches {
        if !names.insert(patch.name) {
            return Err(format!("Duplicate patch name {}", patch.name));
        }
        groups.entry(patch.resolver).or_default().push(*patch);
    }
    for group in groups.values() {
        for patch in group {
            (patch.validate_group)(group)?;
        }
    }
    Ok(())
}

/// Validate unique patch names and required coverage across each resolver's group.
/// This does not run factories, conditions, or memory writes.
pub fn validate_patch_registrations() -> Result<(), String> {
    validate(&inventory::iter::<PatchRegistration>.into_iter().collect::<Vec<_>>())
}

/// Validate and resolve every applicable patch before invoking factories/writers.
/// Factories must describe edits, not modify native memory themselves.
///
/// # Safety
/// Each resolved address and resulting edit must satisfy its native contract.
/// Earlier writes are not rolled back if a later writer call fails.
pub unsafe fn apply_registered_patches<E: SelectionKey, W: PatchWriter>(
    base: u64, offsets: &HashMap<String, u64>, selection: E, writer: &mut W,
) -> Result<Vec<PatchApplication>, String> {
    let patches: Vec<_> = inventory::iter::<PatchRegistration>.into_iter().collect();
    validate(&patches)?;
    let selected = patches.into_iter().filter(|patch| (patch.applies_to)(&selection))
        .map(|patch| {
            if !crate::supports_selection(patch.resolver, selection) {
                return Err(format!("Patch {} targets unavailable resolver {}", patch.name, patch.resolver));
            }
            let offset = offsets.get(patch.resolver)
                .ok_or_else(|| format!("No address for patch {} resolver {}", patch.name, patch.resolver))?;
            let address = base.checked_add(*offset).ok_or_else(|| format!("Address overflow for patch {}", patch.name))?;
            Ok((patch, address))
        }).collect::<Result<Vec<_>, String>>()?;
    selected.into_iter().map(|(patch, address)| {
        let applied = unsafe { (patch.factory)(address).apply(address, writer) }
            .map_err(|error| format!("Patch {}: {error}", patch.name))?;
        Ok(PatchApplication { name: patch.name, resolver: patch.resolver, applied })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResolverDeclaration;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Key { First, Second }
    impl SelectionKey for Key {
        const REQUIRED: &'static [Self] = &[Self::First, Self::Second];
        fn name(self) -> &'static str { match self { Self::First => "First", Self::Second => "Second" } }
    }

    struct Both;
    impl ResolverDeclaration for Both {
        const NAME: &'static str = "Both";
        const UNRESTRICTED: bool = true;
    }
    impl ResolverSelection<Key> for Both {
        const SELECTIONS: &'static [(Key, bool)] = &[];
    }
    struct FirstOnly;
    impl ResolverDeclaration for FirstOnly {
        const NAME: &'static str = "FirstOnly";
        const UNRESTRICTED: bool = false;
    }
    impl ResolverSelection<Key> for FirstOnly {
        const SELECTIONS: &'static [(Key, bool)] = &[(Key::Second, false)];
    }

    fn never_construct(_: u64) -> Patch { panic!("validation must not construct patches") }
    fn first(key: &dyn std::any::Any) -> bool { crate::__private::selection_matches(key, Key::First) }
    fn second(key: &dyn std::any::Any) -> bool { crate::__private::selection_matches(key, Key::Second) }
    fn both(group: &[&PatchRegistration]) -> Result<(), String> {
        validate_patch_coverage::<Both, _>(Key::First, group)
    }
    static FIRST: PatchRegistration = PatchRegistration {
        name: "first", resolver: "Both", factory: never_construct, applies_to: first, validate_group: both,
    };
    static SECOND: PatchRegistration = PatchRegistration {
        name: "second", resolver: "Both", factory: never_construct, applies_to: second, validate_group: both,
    };
    static FIRST_ONLY: PatchRegistration = PatchRegistration {
        name: "only_first", resolver: "FirstOnly", factory: never_construct, applies_to: first,
        validate_group: |group| validate_patch_coverage::<FirstOnly, _>(Key::First, group),
    };
    static DUPLICATE: PatchRegistration = PatchRegistration {
        name: "first", resolver: "Both", factory: never_construct, applies_to: second, validate_group: both,
    };
    static OTHER_RESOLVER: PatchRegistration = PatchRegistration {
        name: "other", resolver: "Other", factory: never_construct, applies_to: second, validate_group: both,
    };

    #[test_case::test_case(&[&FIRST], Some("required selection Second"); "missing enabled selection")]
    #[test_case::test_case(&[&FIRST, &SECOND], None; "different patch names jointly cover resolver")]
    #[test_case::test_case(&[&FIRST_ONLY], None; "resolver disables second selection")]
    #[test_case::test_case(&[&FIRST, &DUPLICATE], Some("Duplicate patch name first"); "duplicate names")]
    #[test_case::test_case(&[&FIRST, &OTHER_RESOLVER], Some("required selection Second"); "other resolver does not supply coverage")]
    fn coverage_is_grouped_by_resolver(patches: &[&PatchRegistration], expected: Option<&str>) {
        match expected {
            Some(message) => assert!(validate(patches).unwrap_err().contains(message)),
            None => validate(patches).unwrap(),
        }
    }
}
