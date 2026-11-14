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
    fn command_transitions() {
        use Command::*;
        use TaskState::*;

        assert_eq!(TaskState::default(), Paused);
        for (mut state, command, expected, exits) in [
            (Paused, Start, Running, false),
            (Paused, Pause, Paused, false),
            (Paused, Cleanup, Paused, true),
            (Running, Start, Running, false),
            (Running, Pause, Paused, false),
            (Running, Cleanup, Running, true),
            (Halted, Start, Halted, false),
            (Halted, Pause, Halted, false),
            (Halted, Cleanup, Halted, true),
        ] {
            let case = format!("{state:?} + {command:?}");
            assert_eq!(state.apply(command).is_break(), exits, "{case}");
            assert_eq!(state.active(), expected == Running, "{case}");
            assert_eq!(state, expected, "{case}");
        }
    }

    #[test]
    fn halt_is_terminal_from_every_state() {
        for mut state in [TaskState::Paused, TaskState::Running, TaskState::Halted] {
            let case = format!("halt {state:?}");
            state.halt();
            assert!(!state.active(), "{case}");
            assert_eq!(state, TaskState::Halted, "{case}");
        }
    }
}
