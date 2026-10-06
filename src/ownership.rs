//! Who owns a selection, and what each selection event means for us.

use std::{mem, sync::Arc};

use crate::store::Entry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Clipboard,
    Primary,
}

#[derive(Debug, Default)]
pub enum Ownership {
    #[default]
    Empty,
    /// Another client owns the selection and its data is being read.
    Capturing,
    /// The owner vanished while its data was being read.
    Orphaned,
    /// Another client owns the selection; `None` when it must not outlive it.
    Foreign(Option<Arc<Entry>>),
    /// The owner's source died; `entry` is persisted unless a new owner shows
    /// up within the grace. Chrome destroys its old source *before* setting
    /// the new one, so every copy there passes through this state.
    Vacated(Arc<Entry>),
    /// Our own source owns the selection.
    Ours { echo_pending: bool },
}

#[derive(Debug)]
pub enum Reaction {
    Capture,
    /// Persist the vacated entry once the grace elapses, if it is still vacant.
    AwaitVacancy,
    Persist(Arc<Entry>),
    Ignore,
}

impl Ownership {
    pub fn on_selection(&mut self, offered: bool) -> Reaction {
        let (next, reaction) = match (mem::take(self), offered) {
            (Self::Ours { echo_pending: true }, true) => (
                Self::Ours {
                    echo_pending: false,
                },
                Reaction::Ignore,
            ),
            (_, true) => (Self::Capturing, Reaction::Capture),
            (ours @ Self::Ours { echo_pending: true }, false) => (ours, Reaction::Ignore),
            (Self::Capturing, false) => (Self::Orphaned, Reaction::Ignore),
            (Self::Foreign(Some(entry)), false) => (Self::Vacated(entry), Reaction::AwaitVacancy),
            (vacated @ Self::Vacated(_), false) => (vacated, Reaction::Ignore),
            (_, false) => (Self::Empty, Reaction::Ignore),
        };
        *self = next;
        reaction
    }

    /// `entry` is `None` when the capture was secret or failed.
    pub fn on_captured(&mut self, entry: Option<Arc<Entry>>) -> Reaction {
        match mem::take(self) {
            Self::Capturing => {
                *self = Self::Foreign(entry);
                Reaction::Ignore
            }
            Self::Orphaned => match entry {
                Some(entry) => {
                    *self = Self::Vacated(entry);
                    Reaction::AwaitVacancy
                }
                None => Reaction::Ignore,
            },
            unrelated => {
                *self = unrelated;
                Reaction::Ignore
            }
        }
    }

    /// The grace after a vacancy is over: persist if nobody took the selection.
    pub fn on_grace_elapsed(&mut self) -> Reaction {
        match mem::take(self) {
            Self::Vacated(entry) => Reaction::Persist(entry),
            unrelated => {
                *self = unrelated;
                Reaction::Ignore
            }
        }
    }

    pub fn on_claimed(&mut self) {
        *self = Self::Ours { echo_pending: true };
    }

    /// Stops a foreign entry from being persisted once its owner goes away.
    pub fn forget(&mut self, doomed: impl FnOnce(&Entry) -> bool) {
        let is_doomed = match self {
            Self::Foreign(Some(entry)) | Self::Vacated(entry) => doomed(entry),
            _ => false,
        };
        if !is_doomed {
            return;
        }
        match self {
            Self::Foreign(entry) => *entry = None,
            _ => *self = Self::Empty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Arc<Entry> {
        Arc::new(Entry {
            id: 7,
            mimes: vec![("text/plain".into(), Arc::from(&b"hello"[..]))],
        })
    }

    fn captured_foreign() -> Ownership {
        let mut ownership = Ownership::default();
        assert!(matches!(ownership.on_selection(true), Reaction::Capture));
        ownership.on_captured(Some(entry()));
        ownership
    }

    #[test]
    fn foreign_selection_is_captured() {
        let mut ownership = Ownership::default();
        assert!(matches!(ownership.on_selection(true), Reaction::Capture));
        assert!(matches!(ownership, Ownership::Capturing));
    }

    #[test]
    fn echo_of_our_own_source_is_ignored_once() {
        let mut ownership = Ownership::default();
        ownership.on_claimed();
        assert!(matches!(ownership.on_selection(true), Reaction::Ignore));
        assert!(matches!(ownership.on_selection(true), Reaction::Capture));
    }

    fn assert_persists(reaction: Reaction) {
        match reaction {
            Reaction::Persist(persisted) => assert_eq!(persisted.id, 7),
            other => panic!("expected persist, got {other:?}"),
        }
    }

    #[test]
    fn null_after_foreign_persists_the_captured_entry_once_the_grace_elapses() {
        let mut ownership = captured_foreign();
        assert!(matches!(
            ownership.on_selection(false),
            Reaction::AwaitVacancy
        ));
        assert_persists(ownership.on_grace_elapsed());
        assert!(matches!(ownership.on_grace_elapsed(), Reaction::Ignore));
    }

    #[test]
    fn source_swapped_by_its_owner_is_captured_not_reverted() {
        let mut ownership = captured_foreign();
        assert!(matches!(
            ownership.on_selection(false),
            Reaction::AwaitVacancy
        ));
        assert!(matches!(ownership.on_selection(true), Reaction::Capture));
        assert!(matches!(ownership.on_grace_elapsed(), Reaction::Ignore));
    }

    #[test]
    fn null_after_secret_is_not_persisted() {
        let mut ownership = Ownership::default();
        ownership.on_selection(true);
        ownership.on_captured(None);
        assert!(matches!(ownership.on_selection(false), Reaction::Ignore));
    }

    #[test]
    fn null_after_our_own_selection_respects_the_clear() {
        let mut ownership = Ownership::default();
        ownership.on_claimed();
        ownership.on_selection(true);
        assert!(matches!(ownership.on_selection(false), Reaction::Ignore));
        assert!(matches!(ownership, Ownership::Empty));
    }

    #[test]
    fn null_racing_our_claim_keeps_waiting_for_the_echo() {
        let mut ownership = Ownership::default();
        ownership.on_claimed();
        assert!(matches!(ownership.on_selection(false), Reaction::Ignore));
        assert!(matches!(ownership.on_selection(true), Reaction::Ignore));
    }

    #[test]
    fn owner_dying_mid_capture_persists_when_the_capture_lands() {
        let mut ownership = Ownership::default();
        ownership.on_selection(true);
        assert!(matches!(ownership.on_selection(false), Reaction::Ignore));
        assert!(matches!(
            ownership.on_captured(Some(entry())),
            Reaction::AwaitVacancy
        ));
        assert_persists(ownership.on_grace_elapsed());
    }

    #[test]
    fn late_capture_does_not_steal_our_ownership() {
        let mut ownership = Ownership::default();
        ownership.on_selection(true);
        ownership.on_claimed();
        assert!(matches!(
            ownership.on_captured(Some(entry())),
            Reaction::Ignore
        ));
        assert!(matches!(ownership, Ownership::Ours { echo_pending: true }));
    }

    #[test]
    fn forgotten_entries_are_not_persisted() {
        let mut ownership = captured_foreign();
        ownership.forget(|entry| entry.id == 7);
        assert!(matches!(ownership.on_selection(false), Reaction::Ignore));
    }

    #[test]
    fn entries_forgotten_during_the_grace_are_not_persisted() {
        let mut ownership = captured_foreign();
        ownership.on_selection(false);
        ownership.forget(|entry| entry.id == 7);
        assert!(matches!(ownership.on_grace_elapsed(), Reaction::Ignore));
    }
}
