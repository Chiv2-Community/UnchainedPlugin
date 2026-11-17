use std::time::{Duration, Instant};

const INTERVAL: Duration = Duration::from_secs(30 * 60);

#[derive(Default)]
pub(super) struct Schedule {
    is_match_live: bool,
    next_broadcast: Option<Instant>,
}

impl Schedule {
    pub(super) fn set_match_state(&mut self, state: &str) {
        self.is_match_live = state == "InProgress";
        if !self.is_match_live {
            self.next_broadcast = None;
        }
    }

    pub(super) fn should_broadcast(&mut self, now: Instant, is_world_ready: bool) -> bool {
        if !self.is_match_live
            || !is_world_ready
            || self.next_broadcast.is_some_and(|next| now < next)
        {
            return false;
        }
        self.next_broadcast = Some(now + INTERVAL);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_does_not_broadcast_even_with_a_world() {
        assert!(!Schedule::default().should_broadcast(Instant::now(), true));
    }

    #[test]
    fn waiting_match_does_not_broadcast() {
        let mut schedule = Schedule::default();
        schedule.set_match_state("WaitingToStart");
        assert!(!schedule.should_broadcast(Instant::now(), true));
    }

    #[test]
    fn preparing_match_does_not_broadcast() {
        let mut schedule = Schedule::default();
        schedule.set_match_state("PreparingMatch");
        assert!(!schedule.should_broadcast(Instant::now(), true));
    }

    #[test]
    fn first_live_tick_broadcasts_then_waits_thirty_minutes() {
        let now = Instant::now();
        let mut schedule = Schedule::default();
        schedule.set_match_state("InProgress");
        assert!(schedule.should_broadcast(now, true));
        assert!(!schedule.should_broadcast(now, true));
        assert!(!schedule.should_broadcast(now + INTERVAL - Duration::from_secs(1), true));
        assert!(schedule.should_broadcast(now + INTERVAL, true));
    }

    #[test]
    fn missing_world_does_not_consume_first_broadcast() {
        let now = Instant::now();
        let mut schedule = Schedule::default();
        schedule.set_match_state("InProgress");
        assert!(!schedule.should_broadcast(now, false));
        assert!(schedule.should_broadcast(now, true));
    }

    #[test]
    fn repeated_live_notification_does_not_reset_interval() {
        let now = Instant::now();
        let mut schedule = Schedule::default();
        schedule.set_match_state("InProgress");
        assert!(schedule.should_broadcast(now, true));
        schedule.set_match_state("InProgress");
        assert!(!schedule.should_broadcast(now, true));
    }

    #[test]
    fn leaving_match_suppresses_overdue_broadcasts() {
        let now = Instant::now();
        let mut schedule = Schedule::default();
        schedule.set_match_state("InProgress");
        assert!(schedule.should_broadcast(now, true));
        schedule.set_match_state("LeavingMap");
        assert!(!schedule.should_broadcast(now + INTERVAL, true));
        schedule.set_match_state("InProgress");
        assert!(schedule.should_broadcast(now + INTERVAL, true));
    }

    #[test]
    fn map_reset_requires_a_new_live_match() {
        let now = Instant::now();
        let mut schedule = Schedule::default();
        schedule.set_match_state("InProgress");
        assert!(schedule.should_broadcast(now, true));
        schedule = Schedule::default();
        assert!(!schedule.should_broadcast(now + INTERVAL, true));
    }
}
