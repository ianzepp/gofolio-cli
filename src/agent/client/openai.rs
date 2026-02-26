use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::warn;

use super::ModelEntry;
use crate::agent::types::{AgentError, ChatResponse, Content, ContentBlock, Message, Tool};

const REQUEST_TIMEOUT_SECS: u64 = 120;
const CONNECT_TIMEOUT_SECS: u64 = 10;

#[derive(Clone)]
pub struct OpenAIClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl OpenAIClient {
    pub fn new(api_key: String, base_url: String) -> Result<Self, AgentError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .build()
            .map_err(|e| AgentError::ApiRequest(e.to_string()))?;
        Ok(Self {
            http,
            api_key,
            base_url,
        })
    }

    pub async fn chat(
        &self,
        model: &str,
        max_tokens: u32,
        system: &str,
        messages: &[Message],
        tools: Option<&[Tool]>,
    ) -> Result<ChatResponse, AgentError> {
        let oai_messages = self.translate_messages(system, messages);
        let oai_tools = tools.map(|t| t.iter().map(translate_tool).collect::<Vec<_>>());

        let body = OaiRequest {
            model,
            max_tokens,
            messages: &oai_messages,
            tools: oai_tools.as_deref(),
        };

        let response = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::ApiRequest(e.to_string()))?;

        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|e| AgentError::ApiRequest(e.to_string()))?;

        if status != 200 {
            return Err(AgentError::ApiResponse { status, body: text });
        }

        self.parse_response(&text, model)
    }

    pub async fn fetch_models(&self) -> Vec<ModelEntry> {
        match self.fetch_models_from_api().await {
            Ok(models) if !models.is_empty() => models,
            Ok(_) => {
                warn!("OpenAI-compatible models API returned empty list");
                Vec::new()
            }
            Err(e) => {
                warn!(error = %e, "Failed to fetch models from OpenAI-compatible API");
                Vec::new()
            }
        }
    }

    async fn fetch_models_from_api(&self) -> Result<Vec<ModelEntry>, AgentError> {
        let response = self
            .http
            .get(format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| AgentError::ApiRequest(e.to_string()))?;

        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|e| AgentError::ApiRequest(e.to_string()))?;

        if status != 200 {
            return Err(AgentError::ApiResponse { status, body: text });
        }

        let resp: OaiModelsResponse =
            serde_json::from_str(&text).map_err(|e| AgentError::ApiParse(e.to_string()))?;

        Ok(resp
            .data
            .into_iter()
            .map(|m| ModelEntry {
                display_name: m.id.clone(),
                id: m.id,
            })
            .collect())
    }

    /// Translate Anthropic-format messages to OpenAI chat messages.
    fn translate_messages(&self, system: &str, messages: &[Message]) -> Vec<OaiMessage> {
        let mut out = Vec::with_capacity(messages.len() + 1);

        // System prompt becomes a system message
        if !system.is_empty() {
            out.push(OaiMessage {
                role: "system".to_string(),
                content: Some(system.to_string()),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        for msg in messages {
            match &msg.content {
                Content::Text(text) => {
                    out.push(OaiMessage {
                        role: msg.role.clone(),
                        content: Some(text.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                Content::Blocks(blocks) => {
                    if msg.role == "assistant" {
                        // Assistant message with tool_use blocks → tool_calls array
                        let mut text_parts = Vec::new();
                        let mut tool_calls = Vec::new();

                        for block in blocks {
                            match block {
                                ContentBlock::Text { text } => {
                                    text_parts.push(text.as_str());
                                }
                                ContentBlock::ToolUse { id, name, input } => {
                                    tool_calls.push(OaiToolCall {
                                        id: id.clone(),
                                        r#type: "function".to_string(),
                                        function: OaiFunction {
                                            name: name.clone(),
                                            arguments: serde_json::to_string(input)
                                                .unwrap_or_default(),
                                        },
                                    });
                                }
                                _ => {}
                            }
                        }

                        let content = if text_parts.is_empty() {
                            None
                        } else {
                            Some(text_parts.join(""))
                        };

                        let tool_calls_opt = if tool_calls.is_empty() {
                            None
                        } else {
                            Some(tool_calls)
                        };

                        out.push(OaiMessage {
                            role: "assistant".to_string(),
                            content,
                            tool_calls: tool_calls_opt,
                            tool_call_id: None,
                        });
                    } else if msg.role == "user" {
                        // User message with ToolResult blocks → expand to role:"tool" messages
                        for block in blocks {
                            match block {
                                ContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                    ..
                                } => {
                                    out.push(OaiMessage {
                                        role: "tool".to_string(),
                                        content: Some(content.clone()),
                                        tool_calls: None,
                                        tool_call_id: Some(tool_use_id.clone()),
                                    });
                                }
                                ContentBlock::Text { text } => {
                                    out.push(OaiMessage {
                                        role: "user".to_string(),
                                        content: Some(text.clone()),
                                        tool_calls: None,
                                        tool_call_id: None,
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        out
    }

    /// Parse OpenAI response into Anthropic-format ChatResponse.
    fn parse_response(&self, json: &str, model: &str) -> Result<ChatResponse, AgentError> {
        let resp: OaiChatResponse =
            serde_json::from_str(json).map_err(|e| AgentError::ApiParse(e.to_string()))?;

        let choice = resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AgentError::ApiParse("No choices in response".to_string()))?;

        let mut content = Vec::new();

        // Extract text content
        if let Some(text) = choice.message.content
            && !text.is_empty()
        {
            content.push(ContentBlock::Text { text });
        }

        // Extract tool calls
        if let Some(tool_calls) = choice.message.tool_calls {
            for tc in tool_calls {
                let input: serde_json::Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                content.push(ContentBlock::ToolUse {
                    id: tc.id,
                    name: tc.function.name,
                    input,
                });
            }
        }

        // Translate stop reason
        let stop_reason = match choice.finish_reason.as_deref() {
            Some("stop") => "end_turn".to_string(),
            Some("tool_calls") => "tool_use".to_string(),
            Some("length") => "max_tokens".to_string(),
            Some(other) => other.to_string(),
            None => "end_turn".to_string(),
        };

        let (input_tokens, output_tokens) = resp
            .usage
            .map(|u| (u.prompt_tokens, u.completion_tokens))
            .unwrap_or((0, 0));

        Ok(ChatResponse {
            content,
            model: resp.model.unwrap_or_else(|| model.to_string()),
            stop_reason,
            input_tokens,
            output_tokens,
        })
    }
}

// --- OpenAI wire types ---

#[derive(Serialize)]
struct OaiRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: &'a [OaiMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [OaiToolDef]>,
}

#[derive(Serialize)]
struct OaiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OaiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct OaiToolCall {
    id: String,
    r#type: String,
    function: OaiFunction,
}

#[derive(Serialize, Deserialize)]
struct OaiFunction {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct OaiToolDef {
    r#type: String,
    function: OaiToolDefFunction,
}

#[derive(Serialize)]
struct OaiToolDefFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

fn translate_tool(tool: &Tool) -> OaiToolDef {
    OaiToolDef {
        r#type: "function".to_string(),
        function: OaiToolDefFunction {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: tool.input_schema.clone(),
        },
    }
}

// --- OpenAI response types ---

#[derive(Deserialize)]
struct OaiChatResponse {
    choices: Vec<OaiChoice>,
    model: Option<String>,
    usage: Option<OaiUsage>,
}

#[derive(Deserialize)]
struct OaiChoice {
    message: OaiChoiceMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OaiChoiceMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OaiToolCall>>,
}

#[derive(Deserialize)]
struct OaiUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

#[derive(Deserialize)]
struct OaiModelsResponse {
    data: Vec<OaiModelInfo>,
}

#[derive(Deserialize)]
struct OaiModelInfo {
    id: String,
}
