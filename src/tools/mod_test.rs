use super::{MockFixtureSet, encode_path_segment, extract_lookup_key, query_params};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn encodes_reserved_and_unicode_bytes() {
    assert_eq!(encode_path_segment("../AAPL"), "..%2FAAPL");
    assert_eq!(encode_path_segment("BTC 🚀"), "BTC%20%F0%9F%9A%80");
}

#[test]
fn extracts_lookup_key_for_symbol_based_tools() {
    let input = json!({
        "dataSource": "YAHOO",
        "symbol": "AAPL"
    });
    assert_eq!(
        extract_lookup_key("get_holding_detail", &input),
        Some("YAHOO:AAPL".to_string())
    );
    assert_eq!(
        extract_lookup_key("get_asset_profile", &input),
        Some("YAHOO:AAPL".to_string())
    );
}

#[test]
fn extracts_lookup_key_for_query_and_id_tools() {
    assert_eq!(
        extract_lookup_key("search_assets", &json!({"query":"msft"})),
        Some("msft".to_string())
    );
    assert_eq!(
        extract_lookup_key("get_account_balances", &json!({"id":"acc-1"})),
        Some("acc-1".to_string())
    );
}

#[test]
fn query_params_keeps_string_and_number_values() {
    let input = json!({
        "symbol": "AAPL",
        "limit": 25,
        "page": 2u64,
        "skip": "",
        "ignored": true
    });
    assert_eq!(
        query_params(&input, &["symbol", "limit", "page", "skip", "ignored"]),
        vec![
            ("symbol".to_string(), "AAPL".to_string()),
            ("limit".to_string(), "25".to_string()),
            ("page".to_string(), "2".to_string())
        ]
    );
}

#[test]
fn mock_dispatch_supports_case_insensitive_lookup() {
    let fixtures = MockFixtureSet {
        by_tool: HashMap::from([(
            "search_assets".to_string(),
            json!({
                "AAPL": {"symbol":"AAPL"},
                "MSFT": {"symbol":"MSFT"}
            }),
        )]),
    };

    let value = fixtures
        .dispatch("search_assets", &json!({"query":"msft"}))
        .expect("expected fixture match");

    assert_eq!(value, json!({"symbol":"MSFT"}));
}
