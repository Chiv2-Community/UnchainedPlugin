//! Native command state only; registration lifecycle remains SDK-owned.
use std::ops::ControlFlow;

#[derive(Debug, Clone, Copy)]
pub(super) enum Command {
    Start,
    Pause,
    Cleanup,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum TaskState {
    #[default]
    Paused,
    Running,
    Halted,
}

impl TaskState {
    pub(super) fn active(&self) -> bool {
        *self == Self::Running
    }

    /// Cleanup exits the worker; the SDK owns single-shot deletion.
    pub(super) fn apply(&mut self, command: Command) -> ControlFlow<()> {
        match command {
            Command::Start if *self == Self::Paused => *self = Self::Running,
            Command::Pause if *self == Self::Running => *self = Self::Paused,
            Command::Cleanup => return ControlFlow::Break(()),
            Command::Start | Command::Pause => {}
        }
        ControlFlow::Continue(())
    }

    pub(super) fn halt(&mut self) {
        *self = Self::Halted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_resume_does_not_cleanup() {
        let mut state = TaskState::default();
        assert!(!state.active());
        assert!(state.apply(Command::Start).is_continue());
        assert!(state.active());
        assert!(state.apply(Command::Pause).is_continue());
        assert!(!state.active());
        assert!(state.apply(Command::Start).is_continue());
        assert!(state.active());
    }

    #[test]
    fn start_cannot_revive_halted_manager() {
        let mut state = TaskState::default();
        assert!(state.apply(Command::Start).is_continue());
        state.halt();
        assert!(state.apply(Command::Pause).is_continue());
        assert!(state.apply(Command::Start).is_continue());
        assert!(!state.active());
        assert_eq!(state, TaskState::Halted);
    }

    #[test]
    fn cleanup_exits_from_every_state() {
        for mut state in [TaskState::Paused, TaskState::Running, TaskState::Halted] {
            assert!(state.apply(Command::Cleanup).is_break());
        }
    }

    #[test]
    fn repeated_start_and_pause_are_idempotent() {
        let mut state = TaskState::default();
        assert!(state.apply(Command::Pause).is_continue());
        assert_eq!(state, TaskState::Paused);
        assert!(state.apply(Command::Start).is_continue());
        assert!(state.apply(Command::Start).is_continue());
        assert_eq!(state, TaskState::Running);
        assert!(state.apply(Command::Pause).is_continue());
        assert!(state.apply(Command::Pause).is_continue());
        assert_eq!(state, TaskState::Paused);
    }
}
