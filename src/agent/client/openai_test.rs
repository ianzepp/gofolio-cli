use super::{OpenAIClient, translate_tool};
use crate::agent::types::{AgentError, Content, ContentBlock, Message, Tool};

fn client() -> OpenAIClient {
    OpenAIClient::new("test-key".to_string(), "http://localhost".to_string())
        .expect("expected valid client")
}

#[test]
fn translate_messages_emits_system_user_and_tool_blocks() {
    let c = client();
    let messages = vec![
        Message {
            role: "assistant".to_string(),
            content: Content::Blocks(vec![
                ContentBlock::Text {
                    text: "Let me check".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tool-1".to_string(),
                    name: "get_holdings".to_string(),
                    input: serde_json::json!({"account":"acc-1"}),
                },
            ]),
        },
        Message {
            role: "user".to_string(),
            content: Content::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: "tool-1".to_string(),
                content: "{\"ok\":true}".to_string(),
                is_error: None,
            }]),
        },
    ];

    let translated = c.translate_messages("system prompt", &messages);
    assert_eq!(translated.len(), 3);

    let first = serde_json::to_value(&translated[0]).expect("expected serializable value");
    assert_eq!(first["role"], "system");
    assert_eq!(first["content"], "system prompt");

    let second = serde_json::to_value(&translated[1]).expect("expected serializable value");
    assert_eq!(second["role"], "assistant");
    assert_eq!(second["content"], "Let me check");
    assert_eq!(second["tool_calls"][0]["function"]["name"], "get_holdings");

    let third = serde_json::to_value(&translated[2]).expect("expected serializable value");
    assert_eq!(third["role"], "tool");
    assert_eq!(third["tool_call_id"], "tool-1");
}

#[test]
fn parse_response_maps_finish_reason_and_usage() {
    let c = client();
    let json = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "done",
                    "tool_calls": [
                        {
                            "id":"tc_1",
                            "type":"function",
                            "function":{
                                "name":"get_market_data",
                                "arguments":"{\"symbol\":\"AAPL\"}"
                            }
                        }
                    ]
                },
                "finish_reason":"tool_calls"
            }
        ],
        "model":"gpt-test",
        "usage":{"prompt_tokens":10,"completion_tokens":20}
    })
    .to_string();

    let parsed = c
        .parse_response(&json, "fallback-model")
        .expect("expected valid response");
    assert_eq!(parsed.model, "gpt-test");
    assert_eq!(parsed.stop_reason, "tool_use");
    assert_eq!(parsed.input_tokens, 10);
    assert_eq!(parsed.output_tokens, 20);
    assert_eq!(parsed.content.len(), 2);
}

#[test]
fn parse_response_rejects_empty_choices() {
    let c = client();
    let json = serde_json::json!({
        "choices": [],
        "model": "gpt-test"
    })
    .to_string();

    let err = c
        .parse_response(&json, "fallback-model");
    let err = match err {
        Ok(_) => panic!("expected parse error"),
        Err(err) => err,
    };
    assert!(matches!(err, AgentError::ApiParse(_)));
}

#[test]
fn translate_tool_preserves_name_description_and_schema() {
    let tool = Tool {
        name: "search_assets".to_string(),
        description: "Search for assets".to_string(),
        input_schema: serde_json::json!({
            "type":"object",
            "properties":{"query":{"type":"string"}}
        }),
    };

    let translated = translate_tool(&tool);
    let value = serde_json::to_value(&translated).expect("expected serializable value");
    assert_eq!(value["type"], "function");
    assert_eq!(value["function"]["name"], "search_assets");
    assert_eq!(value["function"]["description"], "Search for assets");
    assert_eq!(value["function"]["parameters"]["type"], "object");
}
