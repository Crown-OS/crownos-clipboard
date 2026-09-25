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
    /// Our own source owns the selection.
    Ours { echo_pending: bool },
}

#[derive(Debug)]
pub enum Reaction {
    Capture,
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
            (Self::Foreign(Some(entry)), false) => (Self::Empty, Reaction::Persist(entry)),
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
            Self::Orphaned => entry.map_or(Reaction::Ignore, Reaction::Persist),
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
        if let Self::Foreign(entry) = self
            && entry.as_deref().is_some_and(doomed)
        {
            *entry = None;
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

    #[test]
    fn null_after_foreign_persists_the_captured_entry() {
        let mut ownership = captured_foreign();
        match ownership.on_selection(false) {
            Reaction::Persist(persisted) => assert_eq!(persisted.id, 7),
            other => panic!("expected persist, got {other:?}"),
        }
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
            Reaction::Persist(_)
        ));
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
}
