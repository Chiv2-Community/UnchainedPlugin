use super::a2s_query;
use a2s::errors::Error;
use tokio::net::UdpSocket;

const INFO: &[u8] =
    b"\xff\xff\xff\xffI\x11Test\0Map\0folder\0Game\0\x00\x00\x02\x20\x00dl\x00\x001.0\0";

#[tokio::test]
async fn decodes_info_without_blocking_the_runtime() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let responder = tokio::spawn(async move {
        let mut request = [0; 64];
        let (_, peer) = server.recv_from(&mut request).await.unwrap();
        assert!(request.starts_with(b"\xff\xff\xff\xffTSource Engine Query\0"));
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
async fn answers_server_challenges() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let responder = tokio::spawn(async move {
        let mut request = [0; 64];
        let (_, peer) = server.recv_from(&mut request).await.unwrap();
        server
            .send_to(b"\xff\xff\xff\xffA\x01\x02\x03\x04", peer)
            .await
            .unwrap();
        let (length, peer) = server.recv_from(&mut request).await.unwrap();
        assert!(request[..length].ends_with(&[1, 2, 3, 4]));
        server.send_to(INFO, peer).await.unwrap();
    });
    assert_eq!(a2s_query::query(address).await.unwrap().name, "Test");
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
