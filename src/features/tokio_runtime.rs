use once_cell::sync::Lazy;
use tokio::runtime::{Runtime, Handle};

pub static TOKIO_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
});

pub static TOKIO_RUNTIME_HANDLE: Lazy<Handle> = Lazy::new(|| {
    TOKIO_RUNTIME.handle().clone()
});
