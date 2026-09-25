//! Which offered mime types carry data, and which of them are one payload.

pub const PASSWORD_HINT: &str = "x-kde-passwordManagerHint";

const TEXT_BY_PREFERENCE: [&str; 5] = [
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "STRING",
    "TEXT",
];

const X11_TARGET_META: [&str; 4] = ["SAVE_TARGETS", "TARGETS", "MULTIPLE", "TIMESTAMP"];

/// The offered mime types, ready to fetch: every group is one payload and its
/// first mime is the one to request.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MimeGroups {
    pub password_hint: bool,
    pub groups: Vec<Vec<String>>,
}

pub fn text_rank(mime: &str) -> Option<usize> {
    TEXT_BY_PREFERENCE
        .iter()
        .position(|alias| alias.eq_ignore_ascii_case(mime))
}

pub fn is_text(mime: &str) -> bool {
    text_rank(mime).is_some()
}

fn is_data(mime: &str) -> bool {
    !X11_TARGET_META.contains(&mime) && mime != PASSWORD_HINT
}

pub fn is_secret(hint: &[u8]) -> bool {
    hint.trim_ascii() == b"secret"
}

/// Text aliases collapse into one leading group in preference order; every
/// other data mime is its own group, in offer order.
pub fn group(mimes: Vec<String>) -> MimeGroups {
    let password_hint = mimes.iter().any(|mime| mime == PASSWORD_HINT);
    let mut unique: Vec<String> = Vec::with_capacity(mimes.len());
    for mime in mimes {
        if is_data(&mime) && !unique.contains(&mime) {
            unique.push(mime);
        }
    }
    let (mut text, other): (Vec<_>, Vec<_>) = unique.into_iter().partition(|mime| is_text(mime));
    text.sort_by_key(|mime| text_rank(mime));
    let groups = Some(text)
        .filter(|text| !text.is_empty())
        .into_iter()
        .chain(other.into_iter().map(|mime| vec![mime]))
        .collect();
    MimeGroups {
        password_hint,
        groups,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(mimes: &[&str]) -> Vec<String> {
        mimes.iter().map(|mime| mime.to_string()).collect()
    }

    #[test]
    fn text_aliases_share_one_group_in_preference_order() {
        let grouped = group(owned(&[
            "TEXT",
            "text/html",
            "STRING",
            "UTF8_STRING",
            "text/plain;charset=utf-8",
            "text/plain",
        ]));
        assert_eq!(
            grouped.groups,
            vec![
                owned(&[
                    "text/plain;charset=utf-8",
                    "UTF8_STRING",
                    "text/plain",
                    "STRING",
                    "TEXT",
                ]),
                owned(&["text/html"]),
            ]
        );
    }

    #[test]
    fn meta_targets_and_duplicates_are_skipped() {
        let grouped = group(owned(&[
            "TARGETS",
            "image/png",
            "SAVE_TARGETS",
            "MULTIPLE",
            "TIMESTAMP",
            "image/png",
        ]));
        assert_eq!(grouped.groups, vec![owned(&["image/png"])]);
        assert!(!grouped.password_hint);
    }

    #[test]
    fn password_hint_is_flagged_not_fetched_as_data() {
        let grouped = group(owned(&["text/plain", PASSWORD_HINT]));
        assert!(grouped.password_hint);
        assert_eq!(grouped.groups, vec![owned(&["text/plain"])]);
    }

    #[test]
    fn only_the_secret_hint_value_is_secret() {
        assert!(is_secret(b"secret"));
        assert!(is_secret(b"secret\n"));
        assert!(!is_secret(b"public"));
        assert!(!is_secret(b""));
    }

    #[test]
    fn text_matching_ignores_case() {
        assert_eq!(text_rank("text/plain;charset=UTF-8"), Some(0));
        assert!(!is_text("text/html"));
    }
}
