#[path = "../../unchained-plugin/src/features/server_browser/a2s_query.rs"]
mod a2s_query;
use a2s::errors::Error;
use std::num::NonZeroUsize;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

const INFO: &[u8] =
    b"\xff\xff\xff\xffI\x11Test\0Map\0folder\0Game\0\x00\x00\x02\x20\x00dl\x00\x001.0\0";

#[derive(Debug)]
enum RefreshSignal {
    Timer,
    Notification,
    Cancellation,
    CancellationAndNotification,
    CancellationAndTimer,
}

#[test_case::test_case(RefreshSignal::Timer, true; "poll_interval")]
#[test_case::test_case(RefreshSignal::Notification, true; "pending_refresh")]
#[test_case::test_case(RefreshSignal::Cancellation, false; "cancelled")]
#[test_case::test_case(RefreshSignal::CancellationAndNotification, false; "cancellation_wins_over_refresh")]
#[test_case::test_case(RefreshSignal::CancellationAndTimer, false; "cancellation_wins_over_timer")]
#[tokio::test(start_paused = true)]
async fn metadata_refresh_wait(signal: RefreshSignal, expected: bool) {
    let shutdown = CancellationToken::new();
    let refresh = tokio::sync::Notify::new();
    let mut interval = Duration::from_secs(5);
    match signal {
        RefreshSignal::Timer => {}
        RefreshSignal::Notification => refresh.notify_one(),
        RefreshSignal::Cancellation => shutdown.cancel(),
        RefreshSignal::CancellationAndNotification => {
            shutdown.cancel();
            refresh.notify_one();
        }
        RefreshSignal::CancellationAndTimer => {
            shutdown.cancel();
            interval = Duration::ZERO;
        }
    }
    let started = tokio::time::Instant::now();
    assert_eq!(
        a2s_query::should_refresh(&shutdown, refresh.notified(), interval).await,
        expected
    );
    let elapsed = if matches!(signal, RefreshSignal::Timer) {
        interval
    } else {
        Duration::ZERO
    };
    assert_eq!(started.elapsed(), elapsed);
}

#[test_case::test_case(false; "notification_during_wait")]
#[test_case::test_case(true; "cancellation_during_wait")]
#[tokio::test(start_paused = true)]
async fn metadata_refresh_wakes_without_waiting_for_timer(cancel: bool) {
    let shutdown = CancellationToken::new();
    let refresh = tokio::sync::Notify::new();
    let started = tokio::time::Instant::now();
    let signal = async {
        tokio::task::yield_now().await;
        if cancel {
            shutdown.cancel();
        } else {
            refresh.notify_one();
        }
    };
    let (ready, ()) = tokio::join!(
        a2s_query::should_refresh(&shutdown, refresh.notified(), Duration::from_secs(5)),
        signal,
    );
    assert_eq!(ready, !cancel);
    assert_eq!(started.elapsed(), Duration::ZERO);
}

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

#[test_case::test_case(1, 0; "single_attempt_success")]
#[test_case::test_case(1, 1; "single_attempt_failure")]
#[test_case::test_case(3, 1; "second_attempt")]
#[test_case::test_case(3, 2; "third_attempt")]
#[test_case::test_case(3, 3; "exhausted")]
#[test_case::test_case(4, 3; "fourth_attempt")]
#[tokio::test]
async fn retries_failed_queries(max_attempts: usize, failures: usize) {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let responder = tokio::spawn(async move {
        let mut request = [0; 64];
        for attempt in 0..(failures + 1).min(max_attempts) {
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
    let result = a2s_query::query_with_retry(
        address,
        NonZeroUsize::new(max_attempts).unwrap(),
        &CancellationToken::new(),
    )
    .await;
    if failures >= max_attempts {
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
    let query = tokio::spawn(async move {
        a2s_query::query_with_retry(address, NonZeroUsize::new(3).unwrap(), &shutdown).await
    });
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
