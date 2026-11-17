//! Task-owned MOTD broadcasts, active only during a live server match.

use std::{cell::RefCell, time::Duration};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub(crate) struct MotdBroadcaster {
    message: String,
    task: Option<JoinHandle<()>>,
    cancellation: CancellationToken,
}

impl MotdBroadcaster {
    pub(crate) fn new(message: String) -> Self {
        Self {
            message,
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
        self.task = Some(
            crate::features::tokio_runtime::TOKIO_RUNTIME.spawn(async move {
                loop {
                    let cancellation = cancellation.clone();
                    let message = message.clone();
                    crate::resolvers::unchained_integration::run_on_game_thread(move || {
                        if cancellation.is_cancelled() {
                            return;
                        }
                        let is_world_ready = crate::globals().world()
                            .is_some_and(|world| unsafe { !(*world).authority_game_mode.is_null() });
                        if is_world_ready {
                            sinfo!("Broadcasting MOTD");
                            crate::game::chivalry2::send_ingame_message_on_game_thread(
                                &message,
                                Some(crate::game::chivalry2::EChatType::ServerSay),
                            );
                        }
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
        args.motd.clone().filter(|_| args.should_broadcast_motd()).map(MotdBroadcaster::new)
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
        let mut broadcaster = MotdBroadcaster::new("Welcome".into());
        assert!(broadcaster.task.is_none());
        broadcaster.start();
        let id = broadcaster.task.as_ref().unwrap().id();
        broadcaster.start();
        assert_eq!(broadcaster.task.as_ref().unwrap().id(), id);
    }

    #[test]
    fn restart_keeps_previous_run_cancelled() {
        let mut broadcaster = MotdBroadcaster::new("Welcome".into());
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
        let mut broadcaster = MotdBroadcaster::new("Welcome".into());
        broadcaster.start();
        let cancellation = broadcaster.cancellation.clone();
        drop(broadcaster);
        assert!(cancellation.is_cancelled());
    }
}
