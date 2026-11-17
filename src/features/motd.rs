//! Game-thread MOTD scheduling, active only during a live server match.

mod schedule;

use schedule::Schedule;
use std::{cell::RefCell, time::Instant};

thread_local! {
    static SCHEDULE: RefCell<Schedule> = RefCell::new(Schedule::default());
}

pub(crate) fn reset() {
    SCHEDULE.with(|schedule| *schedule.borrow_mut() = Schedule::default());
}

pub(crate) fn set_match_state(state: &str) {
    SCHEDULE.with(|schedule| schedule.borrow_mut().set_match_state(state));
}

pub(crate) fn tick() {
    let args = crate::cli_args();
    if !args.is_server() {
        return;
    }
    let Some(message) = args.motd.as_deref() else {
        return;
    };
    let is_world_ready = crate::globals()
        .world()
        .is_some_and(|world| unsafe { !(*world).authority_game_mode.is_null() });
    let should_broadcast = SCHEDULE.with(|schedule| {
        schedule
            .borrow_mut()
            .should_broadcast(Instant::now(), is_world_ready)
    });
    if should_broadcast {
        sinfo!("Broadcasting MOTD");
        // Send synchronously here so map travel cannot intervene after the readiness check.
        crate::game::chivalry2::send_ingame_message_on_game_thread(
            message,
            Some(crate::game::chivalry2::EChatType::ServerSay),
        );
    }
}
