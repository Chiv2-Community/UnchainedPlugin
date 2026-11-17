//! Task-owned MOTD broadcasts, active only during a live server match.

use crate::game::engine::UWorld;
use std::{cell::RefCell, time::Duration};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub(crate) type UWorldAccessor = fn() -> Option<*mut UWorld>;

#[derive(Debug)]
pub(crate) struct MotdBroadcaster {
    message: String,
    world_accessor: UWorldAccessor,
    task: Option<JoinHandle<()>>,
    cancellation: CancellationToken,
}

impl MotdBroadcaster {
    pub(crate) fn new(message: String, world_accessor: UWorldAccessor) -> Self {
        Self {
            message,
            world_accessor,
            task: None,
            cancellation: CancellationToken::new(),
        }
    }

    pub(crate) fn start(&mut self) {
        if self.task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }
        self.stop();
        // Each run has its own token so restarting cannot revive an old queued send.
        self.cancellation = CancellationToken::new();
        let cancellation = self.cancellation.clone();
        let message = self.message.clone();
        let world_accessor = self.world_accessor;
        self.task = Some(
            crate::features::tokio_runtime::TOKIO_RUNTIME.spawn(async move {
                loop {
                    let cancellation = cancellation.clone();
                    let message = message.clone();
                    crate::resolvers::unchained_integration::run_on_game_thread(move || {
                        if cancellation.is_cancelled() {
                            return;
                        }
                        let Some(world) = world_accessor() else {
                            return;
                        };
                        if world.is_null() || unsafe { (*world).authority_game_mode.is_null() } {
                            return;
                        }
                        sinfo!("Broadcasting MOTD");
                        crate::game::chivalry2::send_ingame_message_on_game_thread(
                            world,
                            &message,
                            Some(crate::game::chivalry2::EChatType::ServerSay),
                        );
                    });
                    tokio::time::sleep(Duration::from_secs(30 * 60)).await;
                }
            }),
        );
    }

    pub(crate) fn stop(&mut self) {
        self.cancellation.cancel();
        if let Some(task) = self.task.take() {
            task.abort();
        }
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
        args.motd.clone().filter(|_| args.should_broadcast_motd())
            .map(|message| MotdBroadcaster::new(message, || crate::globals().world()))
    });
}

pub(crate) fn reset() {
    BROADCASTER.with(|broadcaster| {
        if let Some(broadcaster) = broadcaster.borrow_mut().as_mut() {
            broadcaster.stop();
        }
    });
}

pub(crate) fn set_match_state(state: &str) {
    BROADCASTER.with(|broadcaster| {
        if let Some(broadcaster) = broadcaster.borrow_mut().as_mut() {
            if state == "InProgress" {
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
    fn drop_cancels_current_run() {
        let mut broadcaster = MotdBroadcaster::new("Welcome".into(), || None);
        broadcaster.start();
        let cancellation = broadcaster.cancellation.clone();
        drop(broadcaster);
        assert!(cancellation.is_cancelled());
    }
}
