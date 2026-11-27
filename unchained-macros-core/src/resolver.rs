//! Backend-independent resolver contracts and typed discovery.

use crate::{ResolverFilter, SelectorSnapshot};
use std::{
    any::{Any, TypeId},
    num::NonZeroUsize,
};

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

        self.filter
            .as_ref()
            .is_none_or(|filter| snapshot.supports_filter(filter))
    }
}

#[doc(hidden)]

pub trait ResolverInfo: Any + Send + Sync {
    fn name(&self) -> &'static str;

    fn backend(&self) -> TypeId;

    fn identity(&self) -> *const ();

    fn supports(&self, snapshot: &SelectorSnapshot) -> bool;
}

impl<R: AddressResolver> ResolverInfo for Resolver<R> {
    fn name(&self) -> &'static str {

        self.name
    }

    fn backend(&self) -> TypeId {

        TypeId::of::<R>()
    }

    fn identity(&self) -> *const () {

        (self as *const Self).cast()
    }

    fn supports(&self, snapshot: &SelectorSnapshot) -> bool {

        self.supports(snapshot)
    }
}

inventory::collect!(&'static dyn ResolverInfo);

pub fn resolver_inventory_for<R: AddressResolver>() -> impl Iterator<Item = &'static Resolver<R>> {

    inventory::iter::<&'static dyn ResolverInfo>
        .into_iter()
        .filter_map(|entry| (*entry as &dyn Any).downcast_ref())
}
