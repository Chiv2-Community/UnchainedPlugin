//! Nonblocking Discord publication. Each channel has one batching worker, with
//! reserved capacity for interactive messages and a bounded FIFO event-log lane.
//! Dashboard updates replace pending state rather than building a backlog.
use super::{diagnostics, SharedDiscordConfig};
use crate::events::models::GameEvent;
use crate::modules::discord::event_log::{
    batch::{Batch, MAX_CHARACTERS},
    EventLogSubscriber,
};
use futures::FutureExt;
use serenity::all::{ChannelId, CreateEmbed, CreateMessage, EditMessage, Http, MessageId};
use serenity::http::Route;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio::time::Instant;

const MESSAGE_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 256;
const FLUSH_INTERVAL: Duration = Duration::from_secs(1);
const DASHBOARD_INTERVAL: Duration = Duration::from_secs(5);
const SLOW_SEND: Duration = Duration::from_secs(10);
const MESSAGE_BURST: usize = 4;
const MAX_OUTSTANDING: usize = 64;

#[derive(Clone)]
pub struct DiscordOutbox {
    channels: Arc<HashMap<ChannelId, ChannelSender>>,
}

struct ChannelSender {
    messages: mpsc::Sender<Queued<CreateMessage>>,
    events: mpsc::Sender<Queued<GameEvent>>,
    dashboard: watch::Sender<Option<CreateEmbed>>,
    drops: Mutex<(usize, Instant)>,
}

struct ChannelReceiver {
    worker: u64,
    messages: mpsc::Receiver<Queued<CreateMessage>>,
    events: mpsc::Receiver<Queued<GameEvent>>,
    dashboard: watch::Receiver<Option<CreateEmbed>>,
}

#[derive(Debug)]
struct Queued<T> {
    value: T,
    published: Instant,
}

impl<T> Queued<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            published: Instant::now(),
        }
    }
}

enum Delivery {
    Message(CreateMessage),
    Dashboard(Option<MessageId>, CreateEmbed),
}

fn channel() -> (ChannelSender, ChannelReceiver) {
    let (messages, message_rx) = mpsc::channel(MESSAGE_CAPACITY);
    let (events, event_rx) = mpsc::channel(EVENT_CAPACITY);
    let (dashboard, dashboard_rx) = watch::channel(None);
    (
        ChannelSender {
            messages,
            events,
            dashboard,
            drops: Mutex::new((0, Instant::now() - Duration::from_secs(5))),
        },
        ChannelReceiver {
            worker: diagnostics::next_send(),
            messages: message_rx,
            events: event_rx,
            dashboard: dashboard_rx,
        },
    )
}

impl DiscordOutbox {
    pub fn new(http: Arc<Http>, config: SharedDiscordConfig) -> Self {
        let mut channels = HashMap::new();
        for id in [
            config.general_chat_channel_id,
            config.admin_notification_channel_id,
            config.dashboard_channel_id,
            config.event_log_channel_id,
        ]
        .into_iter()
        .flatten()
        {
            channels.entry(id).or_insert_with(|| {
                let (sender, receiver) = channel();
                log::debug!(
                    "Discord worker configured: worker={} channel={}",
                    receiver.worker,
                    id
                );
                let http = http.clone();
                let handle = tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
                    crate::features::tokio_runtime::TOKIO_RUNTIME_HANDLE.clone()
                });
                handle.spawn(run(receiver, move |delivery| {
                    let http = http.clone();
                    async move { deliver(&http, id, delivery).await }
                }));
                sender
            });
        }
        Self {
            channels: Arc::new(channels),
        }
    }

    /// Full queues drop new publications; never wait for HTTP or queue capacity.
    pub fn send(&self, id: Option<ChannelId>, message: CreateMessage) {
        if let Some(sender) = id.and_then(|id| self.channels.get(&id)) {
            if sender.messages.try_send(Queued::new(message)).is_err() {
                sender.dropped("messages");
            }
        }
    }

    pub fn log(&self, id: Option<ChannelId>, event: &GameEvent) {
        if let Some(sender) = id.and_then(|id| self.channels.get(&id)) {
            match sender.events.try_reserve() {
                Ok(permit) => permit.send(Queued::new(event.clone())),
                Err(_) => sender.dropped("events"),
            }
        }
    }

    pub fn dashboard(&self, id: Option<ChannelId>, embed: CreateEmbed) {
        if let Some(sender) = id.and_then(|id| self.channels.get(&id)) {
            sender.dashboard.send_replace(Some(embed));
        }
    }
}

impl ChannelSender {
    fn dropped(&self, kind: &str) {
        let mut drops = self.drops.lock().unwrap();
        drops.0 = drops.0.saturating_add(1);
        if drops.1.elapsed() >= Duration::from_secs(5) {
            crate::swarn!(
                "Discord outbound queue dropped {} publications (latest: {}): full or closed",
                drops.0,
                kind
            );
            *drops = (0, Instant::now());
        }
    }
}

async fn deliver(http: &Http, id: ChannelId, delivery: Delivery) -> Result<MessageId, ()> {
    let started = Instant::now();
    let send = diagnostics::next_send();
    let kind = match &delivery {
        Delivery::Message(_) => "message",
        Delivery::Dashboard(None, _) => "dashboard_create",
        Delivery::Dashboard(Some(_), _) => "dashboard_edit",
    };
    let route = match &delivery {
        Delivery::Dashboard(Some(message), _) => Route::ChannelMessage {
            channel_id: id,
            message_id: *message,
        },
        _ => Route::ChannelMessages { channel_id: id },
    };
    log::debug!(
        "Discord send starting: send={} channel={} kind={}",
        send,
        id,
        kind
    );
    diagnostics::log_bucket(http, route, send, "before_request");
    let request = async {
        match delivery {
            Delivery::Message(message) => id.send_message(http, message).await,
            Delivery::Dashboard(Some(message), embed) => {
                id.edit_message(http, message, EditMessage::new().embed(embed))
                    .await
            }
            Delivery::Dashboard(None, embed) => {
                id.send_message(http, CreateMessage::new().embed(embed))
                    .await
            }
        }
    };
    tokio::pin!(request);
    let result = diagnostics::trace(send, async {
        tokio::select! {
            result = &mut request => result,
            _ = tokio::time::sleep(SLOW_SEND) => {
                crate::swarn!("Discord {} to channel {} still pending after {}s: send={} (not proof of rate limiting)", kind, id, SLOW_SEND.as_secs(), send);
                diagnostics::log_bucket(http, route, send, "pending");
                request.await
            }
        }
    }).await;
    diagnostics::log_bucket(http, route, send, "after_request");
    log::debug!(
        "Discord {} to channel {} completed in {:?}: send={}",
        kind,
        id,
        started.elapsed(),
        send
    );
    match result {
        Ok(message) => Ok(message.id),
        Err(error) => {
            diagnostics::log_error(send, &error);
            crate::swarn!(
                "Discord {} to channel {} failed: send={} {}",
                kind,
                id,
                send,
                error
            );
            Err(())
        }
    }
}

async fn run<F, Fut>(mut rx: ChannelReceiver, mut send: F)
where
    F: FnMut(Delivery) -> Fut,
    Fut: Future<Output = Result<MessageId, ()>> + Send + 'static,
{
    let mut requests = JoinSet::new();
    let mut dashboard_busy = false;
    let mut batch = Batch::default();
    let mut deadline = Instant::now() + FLUSH_INTERVAL;
    let mut dashboard_id = None;
    let mut next_dashboard = Instant::now();
    let mut dashboard_pending = false;
    let mut dashboard_open = true;
    let mut message_burst = 0;
    let worker = rx.worker;
    let mut first_event = None;
    log::debug!("Discord worker started: worker={}", worker);
    loop {
        if rx.messages.is_closed()
            && rx.messages.is_empty()
            && rx.events.is_closed()
            && rx.events.is_empty()
            && !dashboard_open
            && !dashboard_pending
        {
            if !batch.is_empty() {
                send_batch(
                    &mut send,
                    &mut requests,
                    &mut batch,
                    worker,
                    "shutdown",
                    first_event,
                    (rx.events.len(), rx.messages.len()),
                );
            }
            // Only shutdown drains requests; normal collection never awaits HTTP.
            while let Some(result) = requests.join_next().await {
                if let Err(error) = result {
                    crate::swarn!("Discord request task failed during shutdown: {}", error);
                }
            }
            log::debug!("Discord worker stopped: worker={}", worker);
            return;
        }
        tokio::select! {
            biased;
            Some(result) = requests.join_next(), if !requests.is_empty() => {
                match result {
                    Ok((true, result)) => {
                        dashboard_busy = false;
                        if let Ok(id) = result { dashboard_id = Some(id); }
                    }
                    Ok((false, _)) => {}
                    Err(error) => {
                        crate::swarn!("Discord request task failed: worker={} {}; dashboard state may be unavailable", worker, error);
                    }
                }
            }
            _ = tokio::time::sleep_until(deadline), if !batch.is_empty() => {
                send_batch(&mut send, &mut requests, &mut batch, worker, "inactivity", first_event, (rx.events.len(), rx.messages.len()));
                message_burst = 0;
            }
            changed = rx.dashboard.changed(), if dashboard_open => {
                log::debug!("Discord dashboard state received: worker={} coalesced={} closed={}", worker, dashboard_pending, changed.is_err());
                dashboard_open = changed.is_ok();
                if dashboard_open { dashboard_pending = true; }
            }
            _ = tokio::time::sleep_until(next_dashboard), if dashboard_pending && !dashboard_busy => {
                let embed = rx.dashboard.borrow_and_update().clone();
                dashboard_pending = false;
                if let Some(embed) = embed {
                    dashboard_busy = dispatch(&mut send, &mut requests, Delivery::Dashboard(dashboard_id, embed), worker);
                    if !dashboard_busy {
                        dashboard_pending = true;
                    }
                }
                next_dashboard = Instant::now() + DASHBOARD_INTERVAL;
            }
            Some(message) = rx.messages.recv(), if message_burst < MESSAGE_BURST || rx.events.is_empty() => {
                log::debug!("Discord interactive dequeue: worker={} queue_age={:?} interactive_pending={} events_pending={}", worker, message.published.elapsed(), rx.messages.len(), rx.events.len());
                dispatch(&mut send, &mut requests, Delivery::Message(message.value), worker);
                message_burst += 1;
            }
            Some(event) = rx.events.recv() => {
                message_burst = 0;
                let entry = EventLogSubscriber::build_embed(&event.value);
                deadline = Instant::now() + FLUSH_INTERVAL;
                if !batch.fits(&entry) {
                    send_batch(&mut send, &mut requests, &mut batch, worker, "character_limit", first_event, (rx.events.len(), rx.messages.len()));
                }
                if batch.is_empty() {
                    first_event = Some(event.published);
                    log::debug!("Discord batch opened: worker={} first_event_queue_age={:?} events_pending={} flush_in={:?}", worker, event.published.elapsed(), rx.events.len(), deadline.saturating_duration_since(Instant::now()));
                }
                batch.push(entry);
                if batch.is_full() {
                    let reason = if batch.characters() >= MAX_CHARACTERS { "character_limit" } else { "embed_limit" };
                    send_batch(&mut send, &mut requests, &mut batch, worker, reason, first_event, (rx.events.len(), rx.messages.len()));
                }
            }
        }
    }
}

fn send_batch<F, Fut>(
    send: &mut F,
    requests: &mut JoinSet<(bool, Result<MessageId, ()>)>,
    batch: &mut Batch,
    worker: u64,
    reason: &str,
    first: Option<Instant>,
    queued: (usize, usize),
) where
    F: FnMut(Delivery) -> Fut,
    Fut: Future<Output = Result<MessageId, ()>> + Send + 'static,
{
    log::debug!("Discord batch flush: worker={} reason={} embeds={} utf16_units={} oldest_event_age={:?} events_pending={} interactive_pending={}", worker, reason, batch.len(), batch.characters(), first.map(|time| time.elapsed()), queued.0, queued.1);
    dispatch(
        send,
        requests,
        Delivery::Message(CreateMessage::new().embeds(batch.take())),
        worker,
    );
}

fn dispatch<F, Fut>(
    send: &mut F,
    requests: &mut JoinSet<(bool, Result<MessageId, ()>)>,
    delivery: Delivery,
    worker: u64,
) -> bool
where
    F: FnMut(Delivery) -> Fut,
    Fut: Future<Output = Result<MessageId, ()>> + Send + 'static,
{
    if requests.len() >= MAX_OUTSTANDING {
        crate::swarn!(
            "Discord dispatch dropped: worker={} outstanding request limit={}",
            worker,
            MAX_OUTSTANDING
        );
        return false;
    }
    let dashboard = matches!(&delivery, Delivery::Dashboard(..));
    let request = send(delivery);
    requests.spawn(async move {
        let result = std::panic::AssertUnwindSafe(request).catch_unwind().await;
        let result = result.unwrap_or_else(|_| {
            crate::swarn!(
                "Discord request task panicked: worker={} dashboard={}",
                worker,
                dashboard
            );
            Err(())
        });
        (dashboard, result)
    });
    log::debug!(
        "Discord request dispatched: worker={} outstanding={}",
        worker,
        requests.len()
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::models::Join;

    fn event(name: impl Into<String>) -> GameEvent {
        GameEvent::JoinEvent(Join { name: name.into() })
    }

    fn start() -> (
        ChannelSender,
        mpsc::UnboundedReceiver<serde_json::Value>,
        tokio::task::JoinHandle<()>,
    ) {
        let (sender, receiver) = channel();
        let (output, received) = mpsc::unbounded_channel();
        let task = tokio::spawn(run(receiver, move |delivery| {
            let value = match delivery {
                Delivery::Message(message) => serde_json::to_value(message).unwrap(),
                Delivery::Dashboard(id, embed) => serde_json::json!({"dashboard": embed, "id": id}),
            };
            output.send(value).unwrap();
            std::future::ready(Ok(MessageId::new(99)))
        }));
        (sender, received, task)
    }

    #[tokio::test]
    async fn batches_in_fifo_order_and_flushes_tail_on_close() {
        let (sender, mut received, task) = start();
        for i in 0..23 {
            sender
                .events
                .try_send(Queued::new(event(format!("event-{i}:"))))
                .unwrap();
        }
        drop(sender);
        task.await.unwrap();
        let mut descriptions = Vec::new();
        for expected in [10, 10, 3] {
            let value = received.recv().await.unwrap();
            let embeds = value["embeds"].as_array().unwrap();
            assert_eq!(embeds.len(), expected);
            descriptions.extend(
                embeds
                    .iter()
                    .map(|e| e["description"].as_str().unwrap().to_owned()),
            );
        }
        for (i, description) in descriptions.iter().enumerate() {
            assert!(description.contains(&format!("event-{i}:")));
        }
        assert!(received.recv().await.is_none());
    }

    #[tokio::test]
    async fn batches_respect_aggregate_character_limit() {
        let (sender, mut received, task) = start();
        for _ in 0..3 {
            sender
                .events
                .try_send(Queued::new(event("🩸".repeat(1700))))
                .unwrap();
        }
        drop(sender);
        task.await.unwrap();
        for _ in 0..3 {
            let value = received.recv().await.unwrap();
            let embeds = value["embeds"].as_array().unwrap();
            assert_eq!(embeds.len(), 1);
            let e = &embeds[0];
            let units: usize = [
                e["title"].as_str().unwrap(),
                e["description"].as_str().unwrap(),
                e["footer"]["text"].as_str().unwrap(),
            ]
            .iter()
            .map(|s| s.encode_utf16().count())
            .sum();
            assert!(units <= 6000);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn exact_character_limit_flushes_immediately_with_formatted_chat_events() {
        use crate::events::models::{ChatSource, ChatType, GameChatMessage};

        let mut chat = GameChatMessage {
            chat_source: ChatSource::Discord,
            chat_type: ChatType::Global,
            sender: "Player".into(),
            message: String::new(),
        };
        let overhead =
            EventLogSubscriber::build_embed(&GameEvent::GameChatMessageEvent(chat.clone()))
                .characters;
        chat.message = "🩸".repeat((2000 - overhead) / 2);
        if (2000 - overhead) % 2 != 0 {
            chat.message.push('a');
        }
        let event = GameEvent::GameChatMessageEvent(chat);
        assert_eq!(EventLogSubscriber::build_embed(&event).characters, 2000);

        let (sender, mut received, task) = start();
        let started = Instant::now();
        for _ in 0..2 {
            sender.events.try_send(Queued::new(event.clone())).unwrap();
        }
        tokio::task::yield_now().await;
        assert!(received.try_recv().is_err());
        sender.events.try_send(Queued::new(event)).unwrap();
        tokio::task::yield_now().await;
        let value = received
            .try_recv()
            .expect("6000 UTF-16 units must flush without waiting");
        assert_eq!(Instant::now(), started, "no inactivity time has elapsed");
        let embeds = value["embeds"].as_array().unwrap();
        assert_eq!(embeds.len(), 3);
        for embed in embeds {
            let units: usize = [
                embed["title"].as_str().unwrap(),
                embed["description"].as_str().unwrap(),
                embed["footer"]["text"].as_str().unwrap(),
            ]
            .iter()
            .map(|text| text.encode_utf16().count())
            .sum();
            assert_eq!(units, 2000);
        }
        drop(sender);
        task.await.unwrap();
        assert!(received.try_recv().is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn partial_batch_flushes_one_second_after_last_arrival() {
        let (sender, mut received, task) = start();
        sender.events.try_send(Queued::new(event("first"))).unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(900)).await;
        sender
            .events
            .try_send(Queued::new(event("second")))
            .unwrap();
        tokio::task::yield_now().await;
        assert!(received.try_recv().is_err());
        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;
        assert!(
            received.try_recv().is_err(),
            "new arrival must reset the inactivity deadline"
        );
        tokio::time::advance(Duration::from_millis(899)).await;
        tokio::task::yield_now().await;
        assert!(received.try_recv().is_err());
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            received.try_recv().unwrap()["embeds"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        drop(sender);
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn ten_events_flush_immediately_and_next_batch_gets_its_own_idle_timer() {
        let (sender, mut received, task) = start();
        for i in 0..9 {
            sender
                .events
                .try_send(Queued::new(event(format!("{i}:"))))
                .unwrap();
        }
        tokio::task::yield_now().await;
        assert!(received.try_recv().is_err());
        tokio::time::advance(Duration::from_millis(900)).await;
        sender.events.try_send(Queued::new(event("tenth"))).unwrap();
        tokio::task::yield_now().await;
        assert_eq!(
            received.try_recv().unwrap()["embeds"]
                .as_array()
                .unwrap()
                .len(),
            10
        );
        // A long empty interval must not cause the next event to flush early.
        tokio::time::advance(Duration::from_secs(10)).await;
        sender.events.try_send(Queued::new(event("tail"))).unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(999)).await;
        tokio::task::yield_now().await;
        assert!(received.try_recv().is_err());
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            received.try_recv().unwrap()["embeds"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        // A later partial batch also waits a full second after its arrival.
        tokio::time::advance(Duration::from_secs(1)).await;
        sender
            .events
            .try_send(Queued::new(event("next tail")))
            .unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(999)).await;
        tokio::task::yield_now().await;
        assert!(received.try_recv().is_err());
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            received.try_recv().unwrap()["embeds"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        drop(sender);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn blocked_send_bounds_logs_and_reserves_interactive_capacity() {
        let (sender, receiver) = channel();
        let id = ChannelId::new(1);
        let outbox = DiscordOutbox {
            channels: Arc::new(HashMap::from([(id, sender)])),
        };
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        let worker_gate = gate.clone();
        let (output, mut received) = mpsc::unbounded_channel();
        let task = tokio::spawn(run(receiver, move |delivery| {
            let gate = worker_gate.clone();
            let kind = match delivery {
                Delivery::Message(m) => serde_json::to_value(m).unwrap(),
                Delivery::Dashboard(..) => serde_json::json!("dashboard"),
            };
            output.send(kind).unwrap();
            async move {
                gate.acquire().await.unwrap().forget();
                Ok(MessageId::new(99))
            }
        }));
        outbox.send(Some(id), CreateMessage::new().content("blocking"));
        assert_eq!(received.recv().await.unwrap()["content"], "blocking");
        tokio::time::timeout(Duration::from_secs(1), async {
            for _ in 0..EVENT_CAPACITY + 5 {
                outbox.log(Some(id), &event("queued"));
            }
            for _ in 0..MESSAGE_CAPACITY + 5 {
                outbox.send(Some(id), CreateMessage::new().content("interactive"));
            }
            for i in 0..100 {
                outbox.dashboard(Some(id), CreateEmbed::new().title(i.to_string()));
            }
        })
        .await
        .expect("publication must not wait for Discord");
        let sender = &outbox.channels[&id];
        assert_eq!(sender.events.capacity(), 0);
        assert_eq!(sender.messages.capacity(), 0);
        // The first drop is reported immediately; the remaining nine aggregate.
        assert_eq!(sender.drops.lock().unwrap().0, 9);
        // Requests dispatch while the first HTTP future is still blocked.
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }
        let mut dispatched = 1;
        while received.try_recv().is_ok() {
            dispatched += 1;
        }
        assert_eq!(dispatched, MAX_OUTSTANDING);
        assert_eq!(sender.events.capacity(), EVENT_CAPACITY);
        assert_eq!(sender.messages.capacity(), MESSAGE_CAPACITY);
        gate.add_permits(EVENT_CAPACITY + MESSAGE_CAPACITY + 5);
        drop(outbox);
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn dashboard_coalesces_pending_state_and_reuses_message_id() {
        let (sender, mut received, task) = start();
        sender
            .dashboard
            .send_replace(Some(CreateEmbed::new().title("old")));
        sender
            .dashboard
            .send_replace(Some(CreateEmbed::new().title("latest")));
        assert_eq!(
            received.recv().await.unwrap()["dashboard"]["title"],
            "latest"
        );
        sender
            .dashboard
            .send_replace(Some(CreateEmbed::new().title("next")));
        tokio::task::yield_now().await;
        assert!(received.try_recv().is_err());
        tokio::time::advance(DASHBOARD_INTERVAL).await;
        tokio::task::yield_now().await;
        let next = received.try_recv().unwrap();
        assert_eq!(next["dashboard"]["title"], "next");
        assert_eq!(next["id"], "99");
        drop(sender);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn failures_do_not_stop_later_publications_and_empty_shutdown_sends_nothing() {
        let (sender, mut received, task) = start();
        drop(sender);
        task.await.unwrap();
        assert!(received.recv().await.is_none());
        let (sender, receiver) = channel();
        let (output, mut received) = mpsc::unbounded_channel();
        let task = tokio::spawn(run(receiver, move |_| {
            output.send(()).unwrap();
            std::future::ready(Err(()))
        }));
        sender
            .messages
            .try_send(Queued::new(CreateMessage::new().content("first")))
            .unwrap();
        sender
            .messages
            .try_send(Queued::new(CreateMessage::new().content("second")))
            .unwrap();
        drop(sender);
        task.await.unwrap();
        assert_eq!(received.recv().await, Some(()));
        assert_eq!(received.recv().await, Some(()));
        assert_eq!(received.recv().await, None);
    }

    #[tokio::test(start_paused = true)]
    async fn pending_dashboard_does_not_block_messages_or_create_duplicates() {
        let (sender, receiver) = channel();
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        let request_gate = gate.clone();
        let (output, mut received) = mpsc::unbounded_channel();
        let task = tokio::spawn(run(receiver, move |delivery| {
            let dashboard = matches!(&delivery, Delivery::Dashboard(..));
            let value = match delivery {
                Delivery::Message(message) => serde_json::to_value(message).unwrap(),
                Delivery::Dashboard(id, embed) => serde_json::json!({"id": id, "dashboard": embed}),
            };
            output.send(value).unwrap();
            let gate = request_gate.clone();
            async move {
                if dashboard {
                    gate.acquire().await.unwrap().forget();
                }
                Ok(MessageId::new(99))
            }
        }));
        sender
            .dashboard
            .send_replace(Some(CreateEmbed::new().title("first")));
        assert!(received.recv().await.unwrap()["id"].is_null());
        sender
            .dashboard
            .send_replace(Some(CreateEmbed::new().title("latest")));
        sender
            .messages
            .try_send(Queued::new(CreateMessage::new().content("independent")))
            .unwrap();
        assert_eq!(received.recv().await.unwrap()["content"], "independent");
        tokio::time::advance(Duration::from_secs(10)).await;
        tokio::task::yield_now().await;
        assert!(
            received.try_recv().is_err(),
            "must not duplicate a pending dashboard create"
        );
        gate.add_permits(2);
        let update = received.recv().await.unwrap();
        assert_eq!(update["id"], "99");
        assert_eq!(update["dashboard"]["title"], "latest");
        drop(sender);
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_request_does_not_block_same_channel_batches_or_partial_timer() {
        let (slow, slow_rx) = channel();
        let (fast, mut fast_output, fast_task) = start();
        let (output, mut received) = mpsc::unbounded_channel();
        let mut first = true;
        let slow_task = tokio::spawn(run(slow_rx, move |delivery| {
            let delay = std::mem::replace(&mut first, false);
            let output = output.clone();
            async move {
                if delay {
                    tokio::time::sleep(Duration::from_secs(300)).await;
                }
                if let Delivery::Message(message) = delivery {
                    output.send(serde_json::to_value(message).unwrap()).unwrap();
                }
                Ok(MessageId::new(99))
            }
        }));
        for i in 0..20 {
            slow.events
                .try_send(Queued::new(event(format!("{i}:"))))
                .unwrap();
        }
        tokio::task::yield_now().await;
        fast.messages
            .try_send(Queued::new(CreateMessage::new().content("unaffected")))
            .unwrap();
        assert_eq!(fast_output.recv().await.unwrap()["content"], "unaffected");
        // The second full batch completes before the first request returns.
        assert_eq!(
            received.try_recv().unwrap()["embeds"]
                .as_array()
                .unwrap()
                .len(),
            10
        );
        slow.messages
            .try_send(Queued::new(CreateMessage::new().content("same channel")))
            .unwrap();
        assert_eq!(received.recv().await.unwrap()["content"], "same channel");
        slow.events.try_send(Queued::new(event("partial"))).unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(FLUSH_INTERVAL).await;
        tokio::task::yield_now().await;
        assert_eq!(
            received.try_recv().unwrap()["embeds"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        tokio::time::advance(Duration::from_secs(300)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            received.try_recv().unwrap()["embeds"]
                .as_array()
                .unwrap()
                .len(),
            10
        );
        drop(slow);
        drop(fast);
        slow_task.await.unwrap();
        fast_task.await.unwrap();
    }

    #[tokio::test]
    async fn discord_callbacks_never_wait_for_a_blocked_transport() {
        use crate::events::broadcast_message::discord::DiscordBroadcastSubscriber;
        use crate::events::bus::Subscriber;
        use crate::events::models::{
            AdminAlert, ChatSource, ChatType, CommandSource, GameChatMessage,
        };
        use crate::features::discord::DiscordConfig;
        use crate::modules::chat::{ChatRelaySubscriber, DiscordChatSink};
        use crate::modules::command::CommandSubscriber;
        use crate::modules::discord::{
            admin_alert::AdminAlertModule, dashboard::DashboardSubscriber,
        };

        let id = ChannelId::new(1);
        let config = Box::leak(Box::new(DiscordConfig {
            bot_token: "unused".into(),
            admin_role_id: Some(7),
            mention_on_admin: false,
            general_chat_channel_id: Some(id),
            admin_notification_channel_id: Some(id),
            dashboard_channel_id: Some(id),
            event_log_channel_id: Some(id),
        }));
        let (sender, receiver) = channel();
        let outbox = DiscordOutbox {
            channels: Arc::new(HashMap::from([(id, sender)])),
        };
        let (started, mut sending) = mpsc::unbounded_channel();
        let task = tokio::spawn(run(receiver, move |_| {
            started.send(()).unwrap();
            std::future::pending::<Result<MessageId, ()>>()
        }));
        outbox.send(Some(id), CreateMessage::new().content("blocking"));
        sending.recv().await.unwrap();
        let (publisher, _) = crate::test_utils::mock_game_event_publisher();
        let (broadcast, _) = crate::test_utils::mock_broadcaster();
        let mut relay =
            ChatRelaySubscriber::new(vec![Box::new(DiscordChatSink::new(config, outbox.clone()))]);
        let mut alerts = AdminAlertModule::new(outbox.clone(), config, publisher);
        let mut dashboard = DashboardSubscriber::new(outbox.clone(), config);
        let mut logs = EventLogSubscriber::new(outbox.clone(), config);
        let mut broadcasts = DiscordBroadcastSubscriber::new(outbox.clone(), config);
        let mut commands = CommandSubscriber::new(broadcast, publisher);
        commands.set_discord_connection(config, outbox.clone());
        commands.register(crate::modules::say::SayCommand::new(broadcast));
        let mut help = crate::test_utils::mock_command_request(
            crate::test_utils::mock_actor("Admin"),
            "help",
            vec![],
        );
        help.source = CommandSource::Discord;
        tokio::time::timeout(Duration::from_secs(1), async {
            relay
                .on_event(&GameEvent::GameChatMessageEvent(GameChatMessage {
                    chat_source: ChatSource::Game,
                    chat_type: ChatType::Global,
                    sender: "Player".into(),
                    message: "chat".into(),
                }))
                .await;
            alerts
                .on_event(&GameEvent::AdminAlertEvent(AdminAlert {
                    reporter: "Player".into(),
                    reason: "help".into(),
                }))
                .await;
            dashboard.on_event(&event("Player")).await;
            dashboard.on_tick().await;
            logs.on_event(&event("Player")).await;
            broadcasts.on_event(&"broadcast".to_string().into()).await;
            commands
                .on_event(&GameEvent::CommandRequestEvent(help))
                .await;
        })
        .await
        .expect("Discord callbacks must not await network I/O");
        // All publication paths reached their queues while the first send stayed blocked.
        assert_eq!(
            outbox.channels[&id].messages.capacity(),
            MESSAGE_CAPACITY - 4
        );
        assert_eq!(outbox.channels[&id].events.capacity(), EVENT_CAPACITY - 1);
        assert!(outbox.channels[&id].dashboard.borrow().is_some());
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn queued_broadcasts_preserve_the_admin_mention_setting() {
        use crate::events::broadcast::{BroadcastMessage, Notify};
        use crate::events::broadcast_message::discord::DiscordBroadcastSubscriber;
        use crate::events::bus::Subscriber;
        use crate::features::discord::DiscordConfig;
        for mentions in [false, true] {
            let id = ChannelId::new(1);
            let config = Box::leak(Box::new(DiscordConfig {
                bot_token: "unused".into(),
                admin_role_id: Some(7),
                mention_on_admin: mentions,
                general_chat_channel_id: Some(id),
                admin_notification_channel_id: None,
                dashboard_channel_id: None,
                event_log_channel_id: None,
            }));
            let (sender, mut receiver) = channel();
            let outbox = DiscordOutbox {
                channels: Arc::new(HashMap::from([(id, sender)])),
            };
            let mut subscriber = DiscordBroadcastSubscriber::new(outbox, config);
            subscriber
                .on_event(
                    &BroadcastMessage::new()
                        .content("alert")
                        .notify(Notify::Admin),
                )
                .await;
            let value = serde_json::to_value(receiver.messages.try_recv().unwrap().value).unwrap();
            assert_eq!(
                value["content"].as_str().unwrap().contains("<@&7>"),
                mentions
            );
            assert_eq!(
                value["allowed_mentions"]["roles"].as_array().unwrap().len(),
                usize::from(mentions)
            );
        }
    }
}
