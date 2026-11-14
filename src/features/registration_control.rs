//! Native command state only; registration lifecycle remains SDK-owned.
#[derive(Debug, Clone, Copy)]
pub(super) enum Command {
    Start,
    Pause,
    Cleanup,
}

#[derive(Default)]
pub(super) struct TaskState {
    running: bool,
    halted: bool,
    cleaned: bool,
}

impl TaskState {
    pub(super) fn active(&self) -> bool {
        self.running && !self.halted && !self.cleaned
    }

    /// Returns true exactly once when cleanup is requested.
    pub(super) fn apply(&mut self, command: Command) -> bool {
        match command {
            Command::Start => self.running = true,
            Command::Pause => self.running = false,
            Command::Cleanup => {
                self.running = false;
                return !std::mem::replace(&mut self.cleaned, true);
            }
        }
        false
    }

    pub(super) fn halt(&mut self) {
        self.halted = true;
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
}
