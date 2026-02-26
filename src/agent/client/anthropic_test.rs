use super::{fallback_models, parse_response};
use crate::agent::types::{AgentError, ContentBlock};

#[test]
fn fallback_models_contains_expected_entries() {
    let models = fallback_models();
    assert!(!models.is_empty());
    assert!(models.iter().any(|m| m.id == "claude-sonnet-4-6"));
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
