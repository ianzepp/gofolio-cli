use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("API request failed: {0}")]
    ApiRequest(String),
    #[error("API error: HTTP {status}: {body}")]
    ApiResponse { status: u16, body: String },
    #[error("API parse failed: {0}")]
    ApiParse(String),
    #[error("tool loop exceeded {0} rounds")]
    MaxRounds(usize),
}

/// A structured content block in a message or API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },

    #[serde(other)]
    Unknown,
}

/// Message content — either plain text or structured blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// A tool definition passed to the Anthropic API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Content,
}

/// Response from the Anthropic Messages API.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// A record of a tool call for UI display.
#[derive(Debug, Clone)]
pub struct ToolCallRecord {
    pub name: String,
    pub duration_ms: u64,
    pub success: bool,
}
