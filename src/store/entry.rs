//! One captured selection: every mime it was offered as, with its bytes.

use std::sync::Arc;

use crate::{EntrySummary, util::mime, util::text};

pub type EntryId = u64;

/// Mime types in serving preference; text aliases share one allocation.
pub type Payloads = Vec<(String, Arc<[u8]>)>;

const PREVIEW_BYTES: usize = 256;

#[derive(Debug)]
pub struct Entry {
    pub id: EntryId,
    pub mimes: Payloads,
}

impl Entry {
    pub fn payload(&self, mime: &str) -> Option<&Arc<[u8]>> {
        self.mimes
            .iter()
            .find_map(|(offered, payload)| (offered == mime).then_some(payload))
    }

    pub fn preferred(&self) -> Option<(&str, &[u8])> {
        self.mimes
            .first()
            .map(|(mime, payload)| (mime.as_str(), &payload[..]))
    }

    pub fn has_payloads(&self, other: &Payloads) -> bool {
        self.mimes.len() == other.len()
            && self
                .mimes
                .iter()
                .zip(other)
                .all(|((mime, payload), (other_mime, other_payload))| {
                    mime == other_mime && payload == other_payload
                })
    }

    /// Bytes held, counting a payload shared by aliases once.
    pub fn byte_size(&self) -> usize {
        let mut previous: Option<&Arc<[u8]>> = None;
        self.mimes
            .iter()
            .filter(|(_, payload)| {
                let fresh = previous.is_none_or(|previous| !Arc::ptr_eq(previous, payload));
                previous = Some(payload);
                fresh
            })
            .map(|(_, payload)| payload.len())
            .sum()
    }

    pub fn preview(&self) -> &str {
        self.mimes
            .iter()
            .find(|(offered, _)| mime::is_text(offered))
            .map_or("", |(_, payload)| text::utf8_prefix(payload, PREVIEW_BYTES))
    }
}

impl From<&Entry> for EntrySummary {
    fn from(entry: &Entry) -> Self {
        Self {
            id: entry.id,
            mimes: entry.mimes.iter().map(|(mime, _)| mime.clone()).collect(),
            bytes: entry.byte_size() as u64,
            preview: entry.preview().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_entry(text: &str) -> Entry {
        let shared: Arc<[u8]> = Arc::from(text.as_bytes());
        Entry {
            id: 1,
            mimes: vec![
                ("text/plain;charset=utf-8".into(), Arc::clone(&shared)),
                ("UTF8_STRING".into(), shared),
                ("text/html".into(), Arc::from(&b"<b>x</b>"[..])),
            ],
        }
    }

    #[test]
    fn shared_aliases_count_once() {
        assert_eq!(text_entry("hello").byte_size(), 5 + 8);
    }

    #[test]
    fn summary_previews_the_text_payload() {
        let summary = EntrySummary::from(&text_entry("hello"));
        assert_eq!(summary.preview, "hello");
        assert_eq!(summary.mimes.len(), 3);
    }

    #[test]
    fn preferred_is_the_first_mime() {
        let entry = text_entry("hello");
        assert_eq!(
            entry.preferred(),
            Some(("text/plain;charset=utf-8", &b"hello"[..]))
        );
        assert!(entry.payload("image/png").is_none());
    }
}
