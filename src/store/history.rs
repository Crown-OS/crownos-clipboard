//! A bounded, deduplicated history of entries.

use std::{collections::VecDeque, sync::Arc};

use super::{Entry, EntryId, Payloads};

#[derive(Debug)]
pub struct History {
    entries: VecDeque<Arc<Entry>>,
    capacity: usize,
    next_id: EntryId,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            capacity,
            next_id: 1,
        }
    }

    /// Stores `mimes` as the newest entry. Identical content already in the
    /// history is moved to the front instead of stored twice.
    pub fn record(&mut self, mimes: Payloads) -> Arc<Entry> {
        let existing = self
            .entries
            .iter()
            .position(|entry| entry.has_payloads(&mimes))
            .and_then(|index| self.entries.remove(index));
        let entry = existing.unwrap_or_else(|| self.detached(mimes));
        self.entries.push_front(Arc::clone(&entry));
        self.entries.truncate(self.capacity);
        entry
    }

    /// An entry with a fresh id that is not kept in the history.
    pub fn detached(&mut self, mimes: Payloads) -> Arc<Entry> {
        let id = self.next_id;
        self.next_id += 1;
        Arc::new(Entry { id, mimes })
    }

    /// Moves entry `id` to the front.
    pub fn promote(&mut self, id: EntryId) -> Option<Arc<Entry>> {
        let entry = self.entries.remove(self.position(id)?)?;
        self.entries.push_front(Arc::clone(&entry));
        Some(entry)
    }

    pub fn get(&self, id: EntryId) -> Option<&Arc<Entry>> {
        self.entries.get(self.position(id)?)
    }

    pub fn remove(&mut self, id: EntryId) -> Option<Arc<Entry>> {
        self.entries.remove(self.position(id)?)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Entry>> {
        self.entries.iter()
    }

    fn position(&self, id: EntryId) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Payloads {
        vec![("text/plain".into(), Arc::from(value.as_bytes()))]
    }

    fn ids(history: &History) -> Vec<EntryId> {
        history.iter().map(|entry| entry.id).collect()
    }

    #[test]
    fn oldest_entries_are_evicted() {
        let mut history = History::new(2);
        for value in ["a", "b", "c"] {
            history.record(text(value));
        }
        assert_eq!(ids(&history), [3, 2]);
    }

    #[test]
    fn identical_content_bumps_instead_of_duplicating() {
        let mut history = History::new(10);
        let first = history.record(text("a"));
        history.record(text("b"));
        let again = history.record(text("a"));
        assert!(Arc::ptr_eq(&first, &again));
        assert_eq!(ids(&history), [1, 2]);
    }

    #[test]
    fn different_mime_sets_are_distinct() {
        let mut history = History::new(10);
        history.record(text("a"));
        history.record(vec![("text/html".into(), Arc::from(&b"a"[..]))]);
        assert_eq!(history.iter().count(), 2);
    }

    #[test]
    fn zero_capacity_keeps_nothing_but_still_hands_out_entries() {
        let mut history = History::new(0);
        let entry = history.record(text("a"));
        assert_eq!(entry.id, 1);
        assert_eq!(history.iter().count(), 0);
    }

    #[test]
    fn promote_remove_and_clear() {
        let mut history = History::new(10);
        for value in ["a", "b", "c"] {
            history.record(text(value));
        }
        assert_eq!(history.promote(1).map(|entry| entry.id), Some(1));
        assert_eq!(ids(&history), [1, 3, 2]);
        assert!(history.remove(3).is_some());
        assert!(history.remove(3).is_none());
        assert_eq!(ids(&history), [1, 2]);
        history.clear();
        assert!(history.get(1).is_none());
    }
}
