//! Bounded native-async A2S metadata lookup.
use a2s::{
    errors::{Error, Result},
    info::Info,
    A2SClient,
};
use backon::{ConstantBuilder, Retryable};
use std::{net::SocketAddr, num::NonZeroUsize, time::Duration};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

/// Wait for a metadata refresh; cancellation takes priority over ready wakeups.
pub(super) async fn wait_for_refresh(
    shutdown: &CancellationToken,
    refresh: &Notify,
    interval: Duration,
) -> bool {
    tokio::select! {
        biased;
        _ = shutdown.cancelled() => false,
        _ = refresh.notified() => true,
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
