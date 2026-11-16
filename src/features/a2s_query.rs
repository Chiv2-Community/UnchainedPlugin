//! Bounded native-async A2S metadata lookup.
use a2s::{
    errors::{Error, Result},
    info::Info,
    A2SClient,
};
use std::{net::SocketAddr, time::Duration};
use tokio_util::sync::CancellationToken;

pub(super) async fn query_with_retry(
    address: SocketAddr,
    shutdown: &CancellationToken,
) -> Result<Option<Info>> {
    let mut attempts = 3;
    loop {
        let result = tokio::select! {
            biased;
            _ = shutdown.cancelled() => return Ok(None),
            result = query(address) => result,
        };
        match result {
            Ok(info) => return Ok(Some(info)),
            Err(error) => {
                attempts -= 1;
                if attempts == 0 {
                    return Err(error);
                }
            }
        }
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => return Ok(None),
            _ = tokio::time::sleep(Duration::from_millis(500)) => {},
        }
    }
}

pub(super) async fn query(address: SocketAddr) -> Result<Info> {
    let client = A2SClient::new().await?;
    tokio::time::timeout(Duration::from_secs(5), client.info(address))
        .await
        .map_err(|_| Error::ErrTimeout)?
}
