//! Native command state only; registration lifecycle remains SDK-owned.
#[derive(Debug, Clone, Copy)]
pub(super) enum Command {
    Start,
    Pause,
    Cleanup,
}

#[derive(Default, PartialEq, Eq)]
pub(super) enum TaskState {
    #[default]
    Paused,
    Running,
    Halted,
    Cleaned,
}

impl TaskState {
    pub(super) fn active(&self) -> bool {
        *self == Self::Running
    }

    /// Returns true exactly once when cleanup is requested.
    pub(super) fn apply(&mut self, command: Command) -> bool {
        match command {
            Command::Start if *self == Self::Paused => *self = Self::Running,
            Command::Pause if *self == Self::Running => *self = Self::Paused,
            Command::Cleanup => {
                return std::mem::replace(self, Self::Cleaned) != Self::Cleaned;
            }
            Command::Start | Command::Pause => {}
        }
        false
    }

    pub(super) fn halt(&mut self) {
        if *self != Self::Cleaned {
            *self = Self::Halted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_resume_does_not_cleanup() {
        let mut state = TaskState::default();
        assert!(!state.active());
        assert!(!state.apply(Command::Start));
        assert!(state.active());
        assert!(!state.apply(Command::Pause));
        assert!(!state.active());
        assert!(!state.apply(Command::Start));
        assert!(state.active());
    }

    #[test]
    fn start_cannot_revive_halted_or_cleaned_manager() {
        let mut state = TaskState::default();
        state.apply(Command::Start);
        state.halt();
        state.apply(Command::Pause);
        state.apply(Command::Start);
        assert!(!state.active());
        assert!(state.apply(Command::Cleanup));
        state.apply(Command::Start);
        assert!(!state.active());
        assert!(!state.apply(Command::Cleanup));
    }

    #[test]
    fn cleanup_before_start_is_terminal_and_single_shot() {
        let mut state = TaskState::default();
        assert!(state.apply(Command::Cleanup));
        state.apply(Command::Start);
        assert!(!state.active());
        assert!(!state.apply(Command::Cleanup));
    }

    #[test]
    fn pause_cannot_reset_terminal_states() {
        for terminal in [TaskState::Halted, TaskState::Cleaned] {
            let needs_cleanup = terminal != TaskState::Cleaned;
            let mut state = terminal;
            assert!(!state.apply(Command::Pause));
            assert!(!state.apply(Command::Start));
            assert!(!state.active());
            assert_eq!(state.apply(Command::Cleanup), needs_cleanup);
            assert!(!state.apply(Command::Cleanup));
        }
    }
}
