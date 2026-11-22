//! Backend-independent resolver contracts and typed discovery.
use std::{any::{Any, TypeId}, num::NonZeroUsize};
use crate::{ResolverFilter, SelectorSnapshot};

pub trait AddressResolver: Send + Sync + 'static {
    type Context;
    /// # Safety
    /// The host must satisfy the backend's readiness, threading, and ABI contracts.
    unsafe fn resolve(&self, context: &Self::Context) -> Result<NonZeroUsize, String>;
}

pub struct Resolver<R: AddressResolver> {
    pub name: &'static str,
    pub implementation: R,
    pub filter: Option<ResolverFilter>,
}

impl<R: AddressResolver> Resolver<R> {
    pub fn supports(&self, snapshot: &SelectorSnapshot) -> bool {
        self.filter.as_ref().is_none_or(|filter| snapshot.supports_filter(filter))
    }
}

#[doc(hidden)]
pub trait ResolverInfo: Send + Sync {
    fn name(&self) -> &'static str;
    fn backend(&self) -> TypeId;
    fn identity(&self) -> *const ();
    fn supports(&self, snapshot: &SelectorSnapshot) -> bool;
}
impl<R: AddressResolver> ResolverInfo for Resolver<R> {
    fn name(&self) -> &'static str { self.name }
    fn backend(&self) -> TypeId { TypeId::of::<R>() }
    fn identity(&self) -> *const () { (self as *const Self).cast() }
    fn supports(&self, snapshot: &SelectorSnapshot) -> bool { self.supports(snapshot) }
}

#[doc(hidden)]
pub struct ErasedResolverRegistration {
    pub info: &'static dyn ResolverInfo,
    value: &'static (dyn Any + Send + Sync),
}
impl ErasedResolverRegistration {
    pub const fn new<R: AddressResolver>(resolver: &'static Resolver<R>) -> Self {
        Self { info: resolver, value: resolver }
    }
}
inventory::collect!(ErasedResolverRegistration);

pub fn resolver_inventory_for<R: AddressResolver>() -> impl Iterator<Item = &'static Resolver<R>> {
    inventory::iter::<ErasedResolverRegistration>.into_iter().filter_map(|entry| entry.value.downcast_ref())
}
