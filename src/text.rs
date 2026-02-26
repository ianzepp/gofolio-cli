/// Truncate a string by bytes while preserving UTF-8 boundaries.
pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::truncate_utf8;

    #[test]
    fn keeps_ascii_prefix() {
        assert_eq!(truncate_utf8("abcdef", 3), "abc");
    }

    #[test]
    fn does_not_split_utf8_codepoint() {
        let s = "BTC 🚀 gains";
        assert_eq!(truncate_utf8(s, 7), "BTC ");
        assert_eq!(truncate_utf8(s, 8), "BTC 🚀");
    }
}
