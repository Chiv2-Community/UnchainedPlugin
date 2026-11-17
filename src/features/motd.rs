//! Task-owned MOTD broadcasts, active only during a live server match.

use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub(crate) struct MotdBroadcaster {
    message: String,
    task: Option<JoinHandle<()>>,
    is_active: Arc<AtomicBool>,
}

impl MotdBroadcaster {
    pub(crate) fn new(message: String) -> Self {
        Self {
            message,
            task: None,
            is_active: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn start(&mut self) {
        if self.task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }
        self.stop();
        // Each run has its own flag so restarting cannot revive an old queued send.
        self.is_active = Arc::new(AtomicBool::new(true));
        let is_active = self.is_active.clone();
        let message = self.message.clone();
        self.task = Some(
            crate::features::tokio_runtime::TOKIO_RUNTIME.spawn(async move {
                loop {
                    let is_active = is_active.clone();
                    let message = message.clone();
                    crate::resolvers::unchained_integration::run_on_game_thread(move || {
                        if !is_active.load(Ordering::Acquire) {
                            return;
                        }
                        let is_world_ready = crate::globals().world().is_some_and(|world| unsafe {
                            !(*world).authority_game_mode.is_null()
                        });
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
        self.is_active.store(false, Ordering::Release);
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
    fn restart_keeps_previous_run_inactive() {
        let mut broadcaster = MotdBroadcaster::new("Welcome".into());
        broadcaster.start();
        let old_run = broadcaster.is_active.clone();
        broadcaster.stop();
        assert!(broadcaster.task.is_none());
        broadcaster.start();
        assert!(!old_run.load(Ordering::Acquire));
        assert!(broadcaster.is_active.load(Ordering::Acquire));
    }

    #[test]
    fn drop_deactivates_current_run() {
        let mut broadcaster = MotdBroadcaster::new("Welcome".into());
        broadcaster.start();
        let is_active = broadcaster.is_active.clone();
        drop(broadcaster);
        assert!(!is_active.load(Ordering::Acquire));
    }
}
