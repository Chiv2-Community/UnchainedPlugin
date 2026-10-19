use super::EventLogSubscriber;
use crate::events::models::GameEvent;
use serenity::all::CreateEmbed;
use std::future::Future;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

const MAX_EMBEDS: usize = 10;
const MAX_CHARACTERS: usize = 6000;
const MAX_DESCRIPTION: usize = 4096;
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);

pub(super) struct LogEmbed {
    pub embed: CreateEmbed,
    pub characters: usize,
}

/// Return the description and its UTF-16 length in one bounded scan.
pub(super) fn truncate_description(mut text: String) -> (String, usize) {
    let mut units = 0;
    let mut ellipsis_boundary = (0, 0);
    for (index, ch) in text.char_indices() {
        units += ch.len_utf16();
        if units < MAX_DESCRIPTION {
            ellipsis_boundary = (index + ch.len_utf8(), units);
        }
        if units > MAX_DESCRIPTION {
            let (end, kept_units) = ellipsis_boundary;
            text.truncate(end);
            text.push('…');
            return (text, kept_units + 1);
        }
    }
    (text, units)
}

fn take_batch(embeds: &mut Vec<CreateEmbed>, characters: &mut usize) -> Vec<CreateEmbed> {
    *characters = 0;
    std::mem::replace(embeds, Vec::with_capacity(MAX_EMBEDS))
}

/// A single worker preserves ordering and lets Serenity handle rate limits without
/// holding up the event bus. Closing the queue drains it and flushes the final batch.
pub(super) async fn run<F, Fut>(mut receiver: mpsc::Receiver<GameEvent>, mut send: F)
where
    F: FnMut(Vec<CreateEmbed>) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut embeds = Vec::with_capacity(MAX_EMBEDS);
    let mut characters = 0;
    let mut deadline = Instant::now() + FLUSH_INTERVAL;

    loop {
        let event = if embeds.is_empty() {
            receiver.recv().await
        } else {
            tokio::select! {
                // Flush on time even when events continuously arrive.
                biased;
                _ = tokio::time::sleep_until(deadline) => {
                    send(take_batch(&mut embeds, &mut characters)).await;
                    continue;
                }
                event = receiver.recv() => event,
            }
        };

        let Some(event) = event else {
            if !embeds.is_empty() {
                send(embeds).await;
            }
            return;
        };
        let entry = EventLogSubscriber::build_embed(&event);
        if characters + entry.characters > MAX_CHARACTERS {
            send(take_batch(&mut embeds, &mut characters)).await;
        }
        if embeds.is_empty() {
            deadline = Instant::now() + FLUSH_INTERVAL;
        }
        characters += entry.characters;
        embeds.push(entry.embed);
        if embeds.len() == MAX_EMBEDS {
            send(take_batch(&mut embeds, &mut characters)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{DROP_REPORT_INTERVAL, QUEUE_CAPACITY};
    use super::*;
    use crate::events::bus::Subscriber;
    use crate::events::models::Join;

    fn join(name: impl Into<String>) -> GameEvent {
        GameEvent::JoinEvent(Join { name: name.into() })
    }

    fn start() -> (
        mpsc::Sender<GameEvent>,
        mpsc::UnboundedReceiver<Vec<CreateEmbed>>,
        tokio::task::JoinHandle<()>,
    ) {
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let (output, batches) = mpsc::unbounded_channel();
        let task = tokio::spawn(run(receiver, move |embeds| {
            output.send(embeds).unwrap();
            std::future::ready(())
        }));
        (sender, batches, task)
    }

    fn description(embed: &CreateEmbed) -> String {
        serde_json::to_value(embed).unwrap()["description"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn descriptions_are_bounded_and_unicode_safe() {
        assert_eq!(
            truncate_description("a".repeat(MAX_DESCRIPTION)),
            ("a".repeat(MAX_DESCRIPTION), MAX_DESCRIPTION)
        );
        let (ascii, ascii_units) = truncate_description("a".repeat(MAX_DESCRIPTION + 1));
        assert_eq!(ascii.encode_utf16().count(), MAX_DESCRIPTION);
        assert_eq!(ascii_units, ascii.encode_utf16().count());
        assert!(ascii.ends_with('…'));
        let (unicode, unicode_units) = truncate_description("🩸".repeat(MAX_DESCRIPTION));
        assert!(unicode.encode_utf16().count() <= MAX_DESCRIPTION);
        assert_eq!(unicode_units, unicode.encode_utf16().count());
        assert!(unicode.ends_with('…'));
        let entry = EventLogSubscriber::build_embed(&join("🩸".repeat(MAX_DESCRIPTION)));
        assert!(description(&entry.embed).encode_utf16().count() <= MAX_DESCRIPTION);
        assert!(entry.characters <= MAX_CHARACTERS);
    }

    #[test]
    fn truncation_preserves_exact_limits_and_reports_the_final_length() {
        let cases = [
            String::new(),
            "short 🩸 text".into(),
            "界".repeat(MAX_DESCRIPTION),
            format!("{}🩸", "a".repeat(MAX_DESCRIPTION - 2)),
            format!("{}🩸", "a".repeat(MAX_DESCRIPTION - 1)),
            format!("{}🩸x", "a".repeat(MAX_DESCRIPTION - 2)),
        ];
        for input in cases {
            let original_units = input.encode_utf16().count();
            let (output, units) = truncate_description(input.clone());
            assert_eq!(units, output.encode_utf16().count());
            assert!(units <= MAX_DESCRIPTION);
            if original_units <= MAX_DESCRIPTION {
                assert_eq!(output, input);
                assert_eq!(units, original_units);
            } else {
                assert!(output.ends_with('…'));
                assert!(input.starts_with(output.trim_end_matches('…')));
            }
        }
    }

    #[tokio::test]
    async fn batches_ten_events_in_order_and_flushes_tail_on_close() {
        let (sender, mut batches, task) = start();
        for i in 0..23 {
            sender.send(join(format!("event-{i}:"))).await.unwrap();
        }
        drop(sender);
        task.await.unwrap();
        let mut descriptions = Vec::new();
        for expected_size in [10, 10, 3] {
            let batch = batches.recv().await.unwrap();
            assert_eq!(batch.len(), expected_size);
            descriptions.extend(batch.iter().map(description));
        }
        assert_eq!(descriptions.len(), 23);
        for (i, text) in descriptions.iter().enumerate() {
            assert!(
                text.contains(&format!("event-{i}:")),
                "event {i} out of order: {text:?}"
            );
        }
        assert!(batches.recv().await.is_none());
    }

    #[tokio::test]
    async fn aggregate_character_limit_splits_batches() {
        let (sender, mut batches, task) = start();
        for _ in 0..3 {
            sender.send(join("x".repeat(3500))).await.unwrap();
        }
        drop(sender);
        task.await.unwrap();
        for _ in 0..3 {
            let batch = batches.recv().await.unwrap();
            assert_eq!(batch.len(), 1);
            let value = serde_json::to_value(&batch[0]).unwrap();
            let count = [
                value["title"].as_str().unwrap(),
                value["description"].as_str().unwrap(),
                value["footer"]["text"].as_str().unwrap(),
            ]
            .iter()
            .map(|text| text.encode_utf16().count())
            .sum::<usize>();
            assert!(count <= MAX_CHARACTERS);
        }
        assert!(batches.recv().await.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn deadline_starts_with_first_event_and_is_not_reset_by_arrivals() {
        let (sender, mut batches, task) = start();
        tokio::time::advance(Duration::from_secs(60)).await;
        sender.send(join("first")).await.unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(4)).await;
        sender.send(join("second")).await.unwrap();
        tokio::task::yield_now().await;
        assert!(batches.try_recv().is_err());
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(batches.try_recv().unwrap().len(), 2);
        drop(sender);
        task.await.unwrap();
        assert!(batches.recv().await.is_none());
    }

    #[tokio::test]
    async fn empty_shutdown_sends_nothing() {
        let (sender, mut batches, task) = start();
        drop(sender);
        task.await.unwrap();
        assert!(batches.recv().await.is_none());
    }

    #[tokio::test]
    async fn blocked_discord_send_does_not_block_subscriber_and_queue_is_bounded() {
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let mut subscriber = EventLogSubscriber {
            sender,
            dropped_events: 0,
            last_drop_report: Instant::now(),
        };
        let (started, mut sending) = mpsc::unbounded_channel();
        let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(0));
        let worker_gate = gate.clone();
        let task = tokio::spawn(run(receiver, move |_| {
            let gate = worker_gate.clone();
            started.send(()).unwrap();
            async move {
                gate.acquire().await.unwrap().forget();
            }
        }));
        for _ in 0..MAX_EMBEDS {
            subscriber.on_event(&join("initial")).await;
        }
        sending.recv().await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            for _ in 0..QUEUE_CAPACITY {
                subscriber.on_event(&join("queued")).await;
            }
            subscriber.on_event(&join("discarded")).await;
        })
        .await
        .expect("subscriber must not wait for Discord");
        assert_eq!(subscriber.sender.capacity(), 0);
        assert_eq!(subscriber.dropped_events, 1);
        drop(subscriber);
        // At most one send per queued event, plus the batch already blocked above.
        gate.add_permits(QUEUE_CAPACITY + 1);
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn closed_worker_is_counted_as_dropped() {
        let (sender, receiver) = mpsc::channel(1);
        drop(receiver);
        let mut subscriber = EventLogSubscriber {
            sender,
            dropped_events: 0,
            last_drop_report: Instant::now(),
        };
        subscriber.on_event(&join("lost")).await;
        assert_eq!(subscriber.dropped_events, 1);
        subscriber.on_tick().await;
        assert_eq!(subscriber.dropped_events, 1);
        tokio::time::advance(DROP_REPORT_INTERVAL).await;
        subscriber.on_tick().await;
        assert_eq!(subscriber.dropped_events, 0);
    }
}
