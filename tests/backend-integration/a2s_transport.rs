use super::a2s_query;
use a2s::errors::Error;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

const INFO: &[u8] =
    b"\xff\xff\xff\xffI\x11Test\0Map\0folder\0Game\0\x00\x00\x02\x20\x00dl\x00\x001.0\0";

#[test_case::test_case(false; "direct")]
#[test_case::test_case(true; "challenge")]
#[tokio::test]
async fn decodes_info(challenge: bool) {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let responder = tokio::spawn(async move {
        let mut request = [0; 64];
        let (_, mut peer) = server.recv_from(&mut request).await.unwrap();
        assert!(request.starts_with(b"\xff\xff\xff\xffTSource Engine Query\0"));
        if challenge {
            server
                .send_to(b"\xff\xff\xff\xffA\x01\x02\x03\x04", peer)
                .await
                .unwrap();
            let (length, challenge_peer) = server.recv_from(&mut request).await.unwrap();
            assert!(request[..length].ends_with(&[1, 2, 3, 4]));
            peer = challenge_peer;
        }
        server.send_to(INFO, peer).await.unwrap();
    });
    let info = a2s_query::query(address).await.unwrap();
    assert_eq!(info.name, "Test");
    assert_eq!(info.map, "Map");
    assert_eq!(info.players, 2);
    assert_eq!(info.max_players, 32);
    responder.await.unwrap();
}

#[tokio::test]
async fn rejects_malformed_replies() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let responder = tokio::spawn(async move {
        let mut request = [0; 64];
        let (_, peer) = server.recv_from(&mut request).await.unwrap();
        server.send_to(b"\xff\xff\xff\xff?", peer).await.unwrap();
    });
    assert!(matches!(
        a2s_query::query(address).await,
        Err(Error::InvalidResponse)
    ));
    responder.await.unwrap();
}

#[tokio::test]
async fn bounds_silent_queries_to_five_seconds() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let query = tokio::spawn(a2s_query::query(address));
    let mut request = [0; 64];
    server.recv_from(&mut request).await.unwrap();
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_secs(5)).await;
    assert!(matches!(query.await.unwrap(), Err(Error::ErrTimeout)));
}

#[test_case::test_case(0; "first_attempt")]
#[test_case::test_case(1; "second_attempt")]
#[test_case::test_case(2; "third_attempt")]
#[test_case::test_case(3; "exhausted")]
#[tokio::test]
async fn retries_failed_queries(failures: usize) {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let responder = tokio::spawn(async move {
        let mut request = [0; 64];
        for attempt in 0..=failures.min(2) {
            let (_, peer) = server.recv_from(&mut request).await.unwrap();
            let reply = if attempt < failures {
                b"\xff\xff\xff\xff?".as_slice()
            } else {
                INFO
            };
            server.send_to(reply, peer).await.unwrap();
        }
        server
    });
    let result = a2s_query::query_with_retry(address, &CancellationToken::new()).await;
    if failures == 3 {
        assert!(matches!(result, Err(Error::InvalidResponse)));
    } else {
        assert_eq!(result.unwrap().unwrap().players, 2);
    }
    let server = responder.await.unwrap();
    assert_eq!(
        server.try_recv_from(&mut [0; 64]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test_case::test_case(false; "before_query")]
#[test_case::test_case(true; "during_query")]
#[tokio::test]
async fn cancellation_stops_metadata_lookup(in_flight: bool) {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    if !in_flight {
        shutdown.cancel();
    }
    let cancel = shutdown.clone();
    let query = tokio::spawn(async move { a2s_query::query_with_retry(address, &shutdown).await });
    if in_flight {
        server.recv_from(&mut [0; 64]).await.unwrap();
        cancel.cancel();
    }
    assert!(query.await.unwrap().unwrap().is_none());
    assert_eq!(
        server.try_recv_from(&mut [0; 64]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
