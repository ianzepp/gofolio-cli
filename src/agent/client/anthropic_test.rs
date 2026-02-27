use super::{anthropic_cost_per_token, fallback_models, parse_response};
use crate::agent::types::{AgentError, ContentBlock};

#[test]
fn fallback_models_contains_expected_entries() {
    let models = fallback_models();
    assert!(!models.is_empty());
    let sonnet = models
        .iter()
        .find(|m| m.id == "claude-sonnet-4-6")
        .expect("expected sonnet fallback model");
    assert_eq!(sonnet.input_cost_per_token, Some(0.000003));
    assert_eq!(sonnet.output_cost_per_token, Some(0.000015));
}

#[test]
fn anthropic_cost_per_token_maps_known_models() {
    assert_eq!(
        anthropic_cost_per_token("claude-opus-4-6"),
        Some((Some(0.000005), Some(0.000025)))
    );
    assert_eq!(
        anthropic_cost_per_token("claude-sonnet-4-5-20250929"),
        Some((Some(0.000003), Some(0.000015)))
    );
    assert_eq!(
        anthropic_cost_per_token("claude-haiku-4-5-20251001"),
        Some((Some(0.000001), Some(0.000005)))
    );
    assert_eq!(anthropic_cost_per_token("claude-unknown"), None);
}

#[test]
fn parse_response_filters_unknown_blocks_and_keeps_usage() {
    let json = serde_json::json!({
        "content": [
            {"type":"text","text":"hello"},
            {"type":"something_else"}
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 12,
            "output_tokens": 34
        }
    })
    .to_string();

    let parsed = parse_response(&json).expect("expected valid response");
    assert_eq!(parsed.model, "claude-sonnet-4-6");
    assert_eq!(parsed.stop_reason, "end_turn");
    assert_eq!(parsed.input_tokens, 12);
    assert_eq!(parsed.output_tokens, 34);
    assert_eq!(parsed.content.len(), 1);
    assert!(matches!(parsed.content[0], ContentBlock::Text { .. }));
}

#[test]
fn parse_response_rejects_invalid_json() {
    let err = match parse_response("{invalid json") {
        Ok(_) => panic!("expected parse error"),
        Err(err) => err,
    };
    assert!(matches!(err, AgentError::ApiParse(_)));
}
