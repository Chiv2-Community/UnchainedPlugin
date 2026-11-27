//! Frozen selector snapshots shared by a Patternsleuth evaluation context.

use patternsleuth::resolvers::futures::future::{join_all, BoxFuture};
use patternsleuth::{
    image::Image,
    resolvers::{AsyncContext, DynResolverFactory, Resolution, ResolverFactory},
};

use std::{cell::RefCell, sync::Arc};
use unchained_macros_core::SelectorSnapshot;

pub trait SelectorSnapshotExt {
    fn eval<'env, F, T: Send + Sync>(&self, image: &'env Image<'_>, f: F) -> T
    where
        F: for<'ctx> FnOnce(&'ctx AsyncContext<'env>) -> BoxFuture<'ctx, T> + Send + Sync + 'env;

    fn resolve_many(
        &self,
        image: &Image<'_>,
        resolvers: &[fn() -> &'static DynResolverFactory],
    ) -> Vec<patternsleuth::resolvers::Result<Arc<dyn Resolution>>>;
}

impl SelectorSnapshotExt for SelectorSnapshot {
    fn eval<'env, F, T: Send + Sync>(&self, image: &'env Image<'_>, f: F) -> T
    where
        F: for<'ctx> FnOnce(&'ctx AsyncContext<'env>) -> BoxFuture<'ctx, T> + Send + Sync + 'env,
    {
        let _scope = SnapshotScope(ACTIVE_SNAPSHOT.with(|active| active.replace(Some(self.clone()))));

        patternsleuth::resolvers::eval(image, |ctx| {
            Box::pin(async move {
                context_snapshot(ctx)
                    .await
                    .expect("explicit snapshot seed must be available");

                f(ctx).await
            })
        })
    }

    fn resolve_many(
        &self,
        image: &Image<'_>,
        resolvers: &[fn() -> &'static DynResolverFactory],
    ) -> Vec<patternsleuth::resolvers::Result<Arc<dyn Resolution>>> {
        let factories: Vec<_> = resolvers.iter().map(|resolver| resolver().factory).collect();

        self.eval(image, |ctx| {
            Box::pin(async move { join_all(factories.into_iter().map(|factory| factory(ctx))).await })
        })
    }
}

thread_local! {
    static ACTIVE_SNAPSHOT: RefCell<Option<SelectorSnapshot>> = const { RefCell::new(None) };
}

struct SnapshotScope(Option<SelectorSnapshot>);

impl Drop for SnapshotScope {
    fn drop(&mut self) {
        ACTIVE_SNAPSHOT.with(|active| active.replace(self.0.take()));
    }
}

static SNAPSHOT: ResolverFactory<SelectorSnapshot> = ResolverFactory {
    name: "SelectorSnapshot",
    factory: |_| {
        Box::pin(async {
            ACTIVE_SNAPSHOT
                .with(|active| active.borrow().clone())
                .map_or_else(SelectorSnapshot::capture_registered, Ok)
                .map_err(patternsleuth::resolvers::ResolveError::new_msg)
        })
    },
};

#[doc(hidden)]
pub async fn context_snapshot(ctx: &AsyncContext<'_>) -> patternsleuth::resolvers::Result<Arc<SelectorSnapshot>> {
    ctx.resolve(&SNAPSHOT).await
}
