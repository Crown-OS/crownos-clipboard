//! Text previews of raw payloads.

/// The longest valid UTF-8 prefix of the first `max_bytes` of `bytes`.
pub fn utf8_prefix(bytes: &[u8], max_bytes: usize) -> &str {
    bytes[..bytes.len().min(max_bytes)]
        .utf8_chunks()
        .next()
        .map_or("", |chunk| chunk.valid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_before_a_split_character() {
        assert_eq!(utf8_prefix("héllo".as_bytes(), 2), "h");
        assert_eq!(utf8_prefix("héllo".as_bytes(), 3), "hé");
    }

    #[test]
    fn stops_at_invalid_bytes() {
        assert_eq!(utf8_prefix(b"ok\xffrest", 64), "ok");
    }
}
