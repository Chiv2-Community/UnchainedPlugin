//! Bounded native-async A2S metadata lookup.
use a2s::{
    errors::{Error, Result},
    info::Info,
    A2SClient,
};
use std::{net::SocketAddr, time::Duration};

pub(super) async fn query(address: SocketAddr) -> Result<Info> {
    let client = A2SClient::new().await?;
    tokio::time::timeout(Duration::from_secs(5), client.info(address))
        .await
        .map_err(|_| Error::ErrTimeout)?
}
