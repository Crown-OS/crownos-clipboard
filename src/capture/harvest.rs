//! What a capture still has to fetch, and what it has collected so far.

use std::{mem, sync::Arc};

use crate::{
    store::Payloads,
    util::mime::{self, MimeGroups},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Continue,
    Secret,
}

#[derive(Debug)]
pub struct Harvest {
    checking_secret: bool,
    pending: Vec<Vec<String>>,
    collected: Payloads,
    remaining_bytes: usize,
}

impl Harvest {
    pub fn new(mimes: Vec<String>, max_bytes: usize) -> Self {
        let MimeGroups {
            password_hint,
            mut groups,
        } = mime::group(mimes);
        groups.reverse();
        Self {
            checking_secret: password_hint,
            pending: groups,
            collected: Vec::new(),
            remaining_bytes: max_bytes,
        }
    }

    /// The password hint first, then one mime per payload group.
    pub fn next_mime(&self) -> Option<&str> {
        if self.checking_secret {
            return Some(mime::PASSWORD_HINT);
        }
        self.pending.last()?.first().map(String::as_str)
    }

    pub fn remaining_bytes(&self) -> usize {
        self.remaining_bytes
    }

    /// Takes the payload for the mime [`Self::next_mime`] last returned.
    pub fn accept(&mut self, payload: Vec<u8>) -> Verdict {
        if mem::take(&mut self.checking_secret) {
            return if mime::is_secret(&payload) {
                Verdict::Secret
            } else {
                Verdict::Continue
            };
        }
        let Some(group) = self.pending.pop() else {
            return Verdict::Continue;
        };
        if !payload.is_empty() {
            self.remaining_bytes = self.remaining_bytes.saturating_sub(payload.len());
            let shared: Arc<[u8]> = payload.into();
            self.collected
                .extend(group.into_iter().map(|mime| (mime, Arc::clone(&shared))));
        }
        Verdict::Continue
    }

    pub fn take_payloads(&mut self) -> Option<Payloads> {
        Some(mem::take(&mut self.collected)).filter(|collected| !collected.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn harvest(mimes: &[&str]) -> Harvest {
        Harvest::new(mimes.iter().map(|mime| mime.to_string()).collect(), 100)
    }

    #[test]
    fn password_hint_is_read_first_and_secret_stops_the_capture() {
        let mut harvest = harvest(&["text/plain", mime::PASSWORD_HINT]);
        assert_eq!(harvest.next_mime(), Some(mime::PASSWORD_HINT));
        assert_eq!(harvest.accept(b"secret".to_vec()), Verdict::Secret);
    }

    #[test]
    fn non_secret_hint_continues_with_the_payload() {
        let mut harvest = harvest(&["text/plain", mime::PASSWORD_HINT]);
        assert_eq!(harvest.accept(b"public".to_vec()), Verdict::Continue);
        assert_eq!(harvest.next_mime(), Some("text/plain"));
    }

    #[test]
    fn text_aliases_are_fetched_once_and_share_the_payload() {
        let mut harvest = harvest(&["TEXT", "UTF8_STRING", "image/png"]);
        assert_eq!(harvest.next_mime(), Some("UTF8_STRING"));
        harvest.accept(b"hello".to_vec());
        assert_eq!(harvest.next_mime(), Some("image/png"));
        assert_eq!(harvest.remaining_bytes(), 95);
        harvest.accept(b"png".to_vec());
        assert_eq!(harvest.next_mime(), None);

        let payloads = harvest.take_payloads().unwrap();
        let mimes: Vec<_> = payloads.iter().map(|(mime, _)| mime.as_str()).collect();
        assert_eq!(mimes, ["UTF8_STRING", "TEXT", "image/png"]);
        assert!(Arc::ptr_eq(&payloads[0].1, &payloads[1].1));
    }

    #[test]
    fn empty_payloads_are_not_kept() {
        let mut harvest = harvest(&["text/plain"]);
        harvest.accept(Vec::new());
        assert!(harvest.take_payloads().is_none());
    }
}
