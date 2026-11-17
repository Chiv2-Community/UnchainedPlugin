//! Game-thread MOTD scheduling, active only during a live server match.

use std::{
    cell::Cell,
    time::{Duration, Instant},
};

thread_local! {
    static NEXT_BROADCAST: Cell<Option<Instant>> = const { Cell::new(None) };
}

pub(crate) fn reset() {
    NEXT_BROADCAST.with(|next| next.set(None));
}

pub(crate) fn set_match_state(state: &str) {
    NEXT_BROADCAST.with(|next| {
        if state != "InProgress" {
            next.set(None);
        } else if next.get().is_none() {
            next.set(Some(Instant::now()));
        }
    });
}

pub(crate) fn tick() {
    let args = crate::cli_args();
    let Some(message) = args.motd.as_deref().filter(|_| args.should_broadcast_motd()) else {
        return;
    };
    let is_world_ready = crate::globals()
        .world()
        .is_some_and(|world| unsafe { !(*world).authority_game_mode.is_null() });
    if !is_world_ready {
        return;
    }
    NEXT_BROADCAST.with(|next| {
        let now = Instant::now();
        if next.get().is_none_or(|deadline| now < deadline) {
            return;
        }
        next.set(Some(now + Duration::from_secs(30 * 60)));
        sinfo!("Broadcasting MOTD");
        // Send synchronously here so map travel cannot intervene after the readiness check.
        crate::game::chivalry2::send_ingame_message_on_game_thread(
            message,
            Some(crate::game::chivalry2::EChatType::ServerSay),
        );
    });
}
