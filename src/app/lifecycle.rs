//! Admission and progress for mutually exclusive foreground transitions.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloseIntent {
    Hide,
    Quit,
}

pub(crate) fn close_intent(intent: CloseIntent, can_hide: bool) -> CloseIntent {
    if intent == CloseIntent::Hide && !can_hide {
        CloseIntent::Quit
    } else {
        intent
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Editing,
    ChangingConnection,
    Preparing(CloseIntent),
    Flushing(CloseIntent),
    Quitting,
}

#[derive(Default)]
pub(crate) struct Lifecycle {
    phase: Cell<Phase>,
}
impl Lifecycle {
    pub(crate) fn blocks_authoring(&self) -> bool {
        self.phase.get() != Phase::Editing
    }
    pub(crate) fn closing(&self) -> bool {
        matches!(
            self.phase.get(),
            Phase::Preparing(_) | Phase::Flushing(_) | Phase::Quitting
        )
    }
    pub(crate) fn flushing(&self) -> bool {
        matches!(self.phase.get(), Phase::Flushing(_))
    }
    pub(crate) fn quitting(&self) -> bool {
        self.phase.get() == Phase::Quitting
    }
    pub(crate) fn prepare_close(&self, intent: CloseIntent) -> bool {
        if self.blocks_authoring() {
            return false;
        }
        self.phase.set(Phase::Preparing(intent));
        true
    }
    pub(crate) fn commit_close(&self, intent: CloseIntent) -> bool {
        if !matches!(self.phase.get(), Phase::Editing)
            && self.phase.get() != Phase::Preparing(intent)
        {
            return false;
        }
        self.phase.set(Phase::Flushing(intent));
        true
    }
    pub(crate) fn start_quit(&self) -> bool {
        if self.phase.get() != Phase::Flushing(CloseIntent::Quit) {
            return false;
        }
        self.phase.set(Phase::Quitting);
        true
    }
    pub(crate) fn finish_close(&self) {
        if self.closing() {
            self.phase.set(Phase::Editing);
        }
    }
    pub(crate) fn begin_connection(&self) -> bool {
        if self.blocks_authoring() {
            return false;
        }
        self.phase.set(Phase::ChangingConnection);
        true
    }
    pub(crate) fn finish_connection(&self) {
        if self.phase.get() == Phase::ChangingConnection {
            self.phase.set(Phase::Editing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_freezes_before_any_async_work_and_rejects_overlaps() {
        for intent in [CloseIntent::Hide, CloseIntent::Quit] {
            let lifecycle = Lifecycle::default();
            assert!(lifecycle.prepare_close(intent));
            assert!(lifecycle.closing());
            assert!(lifecycle.blocks_authoring());
            assert!(!lifecycle.begin_connection());
            assert!(!lifecycle.prepare_close(CloseIntent::Hide));
            assert!(!lifecycle.prepare_close(CloseIntent::Quit));
        }
    }

    #[test]
    fn only_the_prepared_intent_can_commit_and_can_commit_only_once() {
        let lifecycle = Lifecycle::default();
        assert!(lifecycle.prepare_close(CloseIntent::Hide));
        assert!(!lifecycle.commit_close(CloseIntent::Quit));
        assert!(lifecycle.commit_close(CloseIntent::Hide));
        assert!(!lifecycle.commit_close(CloseIntent::Hide));
        assert!(!lifecycle.start_quit());
        lifecycle.finish_close();
        assert!(!lifecycle.blocks_authoring());
    }

    #[test]
    fn failed_preparation_flush_and_quit_restore_editing_and_retry() {
        for finish_in_quit in [false, true] {
            let lifecycle = Lifecycle::default();
            assert!(lifecycle.prepare_close(CloseIntent::Quit));
            lifecycle.finish_close();
            assert!(lifecycle.commit_close(CloseIntent::Quit));
            if finish_in_quit {
                assert!(lifecycle.start_quit());
            }
            lifecycle.finish_close();
            assert!(!lifecycle.closing());
            assert!(!lifecycle.blocks_authoring());
            assert!(lifecycle.prepare_close(CloseIntent::Hide));
        }
    }

    #[test]
    fn connection_change_excludes_close_and_cannot_be_canceled_by_close_completion() {
        let lifecycle = Lifecycle::default();
        assert!(lifecycle.begin_connection());
        assert!(lifecycle.blocks_authoring());
        assert!(!lifecycle.closing());
        assert!(!lifecycle.begin_connection());
        assert!(!lifecycle.prepare_close(CloseIntent::Hide));
        assert!(!lifecycle.commit_close(CloseIntent::Quit));
        lifecycle.finish_close();
        assert!(lifecycle.blocks_authoring());
        lifecycle.finish_connection();
        assert!(!lifecycle.blocks_authoring());
        assert!(lifecycle.prepare_close(CloseIntent::Quit));
        lifecycle.finish_connection();
        assert!(lifecycle.closing());
    }

    #[test]
    fn quitting_requires_durable_quit_intent_and_is_idempotent() {
        let lifecycle = Lifecycle::default();
        assert!(!lifecycle.start_quit());
        assert!(lifecycle.commit_close(CloseIntent::Quit));
        assert!(!lifecycle.quitting());
        assert!(lifecycle.start_quit());
        assert!(lifecycle.quitting());
        assert!(!lifecycle.start_quit());
        assert!(!lifecycle.begin_connection());
        assert!(!lifecycle.commit_close(CloseIntent::Hide));
    }
}
