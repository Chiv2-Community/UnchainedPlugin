//! Bounded native-async A2S metadata lookup.
use a2s::{
    errors::{Error, Result},
    info::Info,
    A2SClient,
};
use backon::{ConstantBuilder, Retryable};
use std::{net::SocketAddr, num::NonZeroUsize, time::Duration};
use tokio_util::sync::CancellationToken;

/// Wait until metadata should refresh; return false on cancellation, which takes priority.
pub(super) async fn should_refresh(
    shutdown: &CancellationToken,
    refresh: impl std::future::Future<Output = ()>,
    interval: Duration,
) -> bool {
    tokio::select! {
        biased;
        _ = shutdown.cancelled() => false,
        () = refresh => true,
        _ = tokio::time::sleep(interval) => true,
    }
}

pub(super) async fn query_with_retry(
    address: SocketAddr,
    max_attempts: NonZeroUsize,
    shutdown: &CancellationToken,
) -> Result<Option<Info>> {
    let policy = ConstantBuilder::default()
        .with_delay(Duration::from_millis(500))
        .with_max_times(max_attempts.get() - 1);
    tokio::select! {
        biased;
        _ = shutdown.cancelled() => Ok(None),
        result = (|| query(address)).retry(policy) => result.map(Some),
    }
}

pub(super) async fn query(address: SocketAddr) -> Result<Info> {
    let client = A2SClient::new().await?;
    tokio::time::timeout(Duration::from_secs(5), client.info(address))
        .await
        .map_err(|_| Error::ErrTimeout)?
}
