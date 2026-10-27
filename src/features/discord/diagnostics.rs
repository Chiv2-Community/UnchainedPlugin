//! Observe HTTP separately from queueing. Never log tokens, URLs, request bodies,
//! or arbitrary headers. Bucket snapshots are parsed state, not raw responses.
use serenity::all::Http;
use serenity::http::{Ratelimit, Route};
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::time::Instant;

static NEXT_SEND: AtomicU64 = AtomicU64::new(1);
tokio::task_local! {
    static ACTIVE_SEND: (u64, Instant);
}

pub(crate) fn next_send() -> u64 {
    NEXT_SEND.fetch_add(1, Ordering::Relaxed)
}

pub(crate) async fn trace<F: Future>(id: u64, future: F) -> F::Output {
    ACTIVE_SEND.scope((id, Instant::now()), future).await
}

#[derive(Debug)]
pub(crate) struct HttpObservation {
    status: Option<u16>,
    method: String,
    timeout: bool,
    connect_error: bool,
    body_error: bool,
}

/// Observe statuses before Serenity consumes/retries 429s. Do not retry message
/// bodies here: Serenity alone owns Discord's rate-limit backoff/retry policy.
/// The current native-TLS HTTP/1 client has no protocol-NACK retries to preserve.
pub(crate) fn response_policy(
    observe: impl Fn(HttpObservation) + Send + Sync + 'static,
) -> discord_http::retry::Builder {
    response_policy_for_host("discord.com", observe)
}

fn response_policy_for_host(
    host: &'static str,
    observe: impl Fn(HttpObservation) + Send + Sync + 'static,
) -> discord_http::retry::Builder {
    discord_http::retry::for_host(host).classify_fn(move |exchange| {
        let error = exchange
            .error()
            .and_then(|e| e.downcast_ref::<discord_http::Error>());
        observe(HttpObservation {
            status: exchange.status().map(|s| s.as_u16()),
            method: exchange.method().to_string(),
            timeout: error.is_some_and(|e| e.is_timeout()),
            connect_error: error.is_some_and(|e| e.is_connect()),
            body_error: error.is_some_and(|e| e.is_body()),
        });
        exchange.success()
    })
}

pub(crate) fn log_http(observation: HttpObservation) {
    let (send, elapsed) = ACTIVE_SEND
        .try_with(|&(id, start)| (Some(id), Some(start.elapsed())))
        .unwrap_or((None, None));
    if observation.status == Some(429) {
        swarn!("Discord HTTP 429: rate limited; send={:?} method={} elapsed={:?}; Serenity will apply backoff", send, observation.method, elapsed);
    } else if let Some(status) = observation.status {
        sdebug!(
            "Discord HTTP response headers received: send={:?} method={} status={} elapsed={:?}",
            send,
            observation.method,
            status,
            elapsed
        );
    } else {
        swarn!("Discord HTTP transport failure: send={:?} method={} elapsed={:?} timeout={} connect_error={} body_error={} (not an HTTP 429)", send, observation.method, elapsed, observation.timeout, observation.connect_error, observation.body_error);
    }
}

pub(crate) fn log_error(send: u64, error: &serenity::Error) {
    if let serenity::Error::Http(serenity::http::HttpError::Request(error)) = error {
        swarn!("Discord request failure details: send={} timeout={} connect_error={} body_error={} decode_error={} (no HTTP 429 established by this error)", send, error.is_timeout(), error.is_connect(), error.is_body(), error.is_decode());
    }
}

fn parsed_headers(bucket: &Ratelimit) -> String {
    let limit = (bucket.limit() != i64::MAX).then(|| bucket.limit());
    let remaining = (bucket.remaining() != i64::MAX).then(|| bucket.remaining());
    format!("X-RateLimit-Limit={limit:?} X-RateLimit-Remaining={remaining:?} X-RateLimit-Reset-After={:?}", bucket.reset_after())
}

/// Diagnostic reads must not queue behind the limiter's backoff lock.
pub(crate) fn log_bucket(http: &Http, route: Route<'_>, send: u64, phase: &str) {
    let Some(limiter) = &http.ratelimiter else {
        return;
    };
    let routes = limiter.routes();
    let Ok(routes) = routes.try_read() else {
        sdebug!(
            "Discord rate-limit snapshot: send={} phase={} route_table_busy=true",
            send,
            phase
        );
        return;
    };
    let Some(bucket) = routes.get(&route.ratelimiting_bucket()).cloned() else {
        sdebug!(
            "Discord rate-limit snapshot: send={} phase={} headers_not_seen=true",
            send,
            phase
        );
        return;
    };
    drop(routes);
    match bucket.try_lock() {
        Ok(bucket) => sdebug!("Discord rate-limit snapshot (Serenity parsed/cached state): send={} phase={} {}", send, phase, parsed_headers(&bucket)),
        Err(_) => sdebug!("Discord rate-limit snapshot: send={} phase={} bucket_busy=true (possibly applying backoff)", send, phase),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn response(status: &str) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            socket.read(&mut request).await.unwrap();
            let response = format!("HTTP/1.1 {status}\r\nContent-Length: 2\r\nConnection: close\r\nX-RateLimit-Limit: 5\r\nX-RateLimit-Remaining: 4\r\nX-RateLimit-Reset-After: 2\r\nRetry-After: 0.01\r\nAuthorization: never-log-this\r\n\r\n{{}}");
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        (format!("http://{address}/"), task)
    }

    #[tokio::test]
    async fn observes_real_429_without_retrying_or_logging_request_secrets() {
        let (url, server) = response("429 Too Many Requests").await;
        let observations = Arc::new(Mutex::new(Vec::new()));
        let output = observations.clone();
        let client = discord_http::Client::builder()
            .retry(response_policy_for_host("127.0.0.1", move |o| {
                output.lock().unwrap().push(o)
            }))
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let result = client
            .post(url)
            .bearer_auth("secret-token")
            .body("private chat text")
            .send()
            .await
            .unwrap();
        assert_eq!(result.status().as_u16(), 429);
        server.await.unwrap();
        let observations = observations.lock().unwrap();
        assert_eq!(observations.len(), 1, "only Serenity may retry 429s");
        assert_eq!(observations[0].status, Some(429));
        assert_eq!(observations[0].method, "POST");
        let text = format!("{observations:?}");
        for secret in ["secret-token", "private chat text", "never-log-this"] {
            assert!(!text.contains(secret));
        }
    }

    #[tokio::test]
    async fn snapshot_reports_parsed_response_header_values() {
        let (url, server) = response("200 OK").await;
        let response = super::super::http_client(Duration::from_secs(2))
            .unwrap()
            .get(url)
            .send()
            .await
            .unwrap();
        let mut bucket = Ratelimit::default();
        let request = serenity::http::Request::new(
            Route::ChannelMessages {
                channel_id: serenity::all::ChannelId::new(1),
            },
            serenity::http::LightMethod::Post,
        );
        assert!(!bucket
            .post_hook(&response, &request, &|_| {}, false)
            .await
            .unwrap());
        let text = parsed_headers(&bucket);
        assert!(text.contains("X-RateLimit-Limit=Some(5)"));
        assert!(text.contains("X-RateLimit-Remaining=Some(4)"));
        assert!(text.contains("X-RateLimit-Reset-After=Some(2s)"));
        assert!(!text.contains("never-log-this"));
        server.await.unwrap();
    }

    #[test]
    fn unseen_headers_are_not_reported_as_real_limits() {
        let text = parsed_headers(&Ratelimit::default());
        assert!(text.contains("X-RateLimit-Limit=None"));
        assert!(text.contains("X-RateLimit-Remaining=None"));
    }
}
