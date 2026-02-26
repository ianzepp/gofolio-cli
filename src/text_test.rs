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
