//! Task-owned MOTD broadcasts, active only during a live server match.

use crate::features::tokio_runtime::TOKIO_RUNTIME;
use crate::features::native_chat::send_ingame_message_with_world;
use crate::Chivalry2::TBL::EChatType;
use crate::features::native_chat::UWorldAccessor;
use std::{cell::RefCell, time::Duration};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const BROADCAST_INTERVAL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug)]
struct MotdBroadcaster {
    message: String,
    world_accessor: UWorldAccessor,
    task: Option<JoinHandle<()>>,
    cancellation: CancellationToken,
}

impl MotdBroadcaster {
    fn new(message: String, world_accessor: UWorldAccessor) -> Self {
        Self {
            message,
            world_accessor,
            task: None,
            cancellation: CancellationToken::new(),
        }
    }

    fn start(&mut self) {
        if self.task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }
        self.stop();
        // Each run has its own token so restarting cannot revive an old queued send.
        self.cancellation = CancellationToken::new();
        let cancellation = self.cancellation.clone();
        let message = self.message.clone();
        let world_accessor = self.world_accessor;
        let future = async move {
            cancellation
                .run_until_cancelled(async {
                    loop {
                        send_ingame_message_with_world(
                            message.clone(),
                            Some(EChatType::ServerSay),
                            world_accessor,
                            Some(cancellation.clone()),
                        );
                        tokio::time::sleep(BROADCAST_INTERVAL).await;
                    }
                })
                .await;
        };
        self.task = Some(TOKIO_RUNTIME.spawn(future));
    }

    fn stop(&mut self) {
        self.cancellation.cancel();
        self.task = None;
    }
}

impl Drop for MotdBroadcaster {
    fn drop(&mut self) {
        self.stop();
    }
}

thread_local! {
    static BROADCASTER: RefCell<Option<MotdBroadcaster>> = RefCell::new({
        let args = crate::cli_args();
        args.motd.as_ref()
            .filter(|_| args.should_broadcast_motd())
            .map(|message| MotdBroadcaster::new(message.clone(), || crate::globals().world()))
    });
}

pub(crate) fn set_match_active(is_active: bool) {
    BROADCASTER.with(|broadcaster| {
        if let Some(broadcaster) = broadcaster.borrow_mut().as_mut() {
            if is_active {
                broadcaster.start();
            } else {
                broadcaster.stop();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_start_keeps_one_task() {
        let mut broadcaster = MotdBroadcaster::new("Welcome".into(), || None);
        assert!(broadcaster.task.is_none());
        broadcaster.start();
        let id = broadcaster.task.as_ref().unwrap().id();
        broadcaster.start();
        assert_eq!(broadcaster.task.as_ref().unwrap().id(), id);
    }

    #[test]
    fn restart_keeps_previous_run_cancelled() {
        let mut broadcaster = MotdBroadcaster::new("Welcome".into(), || None);
        broadcaster.start();
        let old_run = broadcaster.cancellation.clone();
        broadcaster.stop();
        assert!(broadcaster.task.is_none());
        broadcaster.start();
        assert!(old_run.is_cancelled());
        assert!(!broadcaster.cancellation.is_cancelled());
    }

    #[test]
    fn cancellation_finishes_the_task_without_aborting_it() {
        let mut broadcaster = MotdBroadcaster::new("Welcome".into(), || None);
        broadcaster.start();
        let task = broadcaster.task.take().unwrap();
        broadcaster.stop();
        TOKIO_RUNTIME.block_on(async {
            tokio::time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap();
        });
    }

    #[test]
    fn drop_cancels_current_run() {
        let mut broadcaster = MotdBroadcaster::new("Welcome".into(), || None);
        broadcaster.start();
        let cancellation = broadcaster.cancellation.clone();
        drop(broadcaster);
        assert!(cancellation.is_cancelled());
    }
}
