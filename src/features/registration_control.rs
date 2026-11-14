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
    use Command::*;
    use TaskState::*;

    #[test]
    fn starts_paused() {
        assert_eq!(TaskState::default(), Paused);
    }

    #[test_case::test_case(Paused, Start, Running, false; "start_paused")]
    #[test_case::test_case(Paused, Pause, Paused, false; "pause_paused")]
    #[test_case::test_case(Paused, Cleanup, Paused, true; "cleanup_paused")]
    #[test_case::test_case(Running, Start, Running, false; "start_running")]
    #[test_case::test_case(Running, Pause, Paused, false; "pause_running")]
    #[test_case::test_case(Running, Cleanup, Running, true; "cleanup_running")]
    #[test_case::test_case(Halted, Start, Halted, false; "start_halted")]
    #[test_case::test_case(Halted, Pause, Halted, false; "pause_halted")]
    #[test_case::test_case(Halted, Cleanup, Halted, true; "cleanup_halted")]
    fn command_transitions(
        mut state: TaskState,
        command: Command,
        expected: TaskState,
        exits: bool,
    ) {
        assert_eq!(state.apply(command).is_break(), exits);
        assert_eq!(state.active(), expected == Running);
        assert_eq!(state, expected);
    }

    #[test_case::test_case(Paused; "paused")]
    #[test_case::test_case(Running; "running")]
    #[test_case::test_case(Halted; "halted")]
    fn halt_is_terminal(mut state: TaskState) {
        state.halt();
        assert!(!state.active());
        assert_eq!(state, Halted);
    }
}
