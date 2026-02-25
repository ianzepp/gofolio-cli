use std::time::Duration;

use tracing::warn;

use super::types::{AgentError, ChatResponse, ContentBlock, Message, Tool};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODELS_URL: &str = "https://api.anthropic.com/v1/models";
const API_VERSION: &str = "2023-06-01";
const REQUEST_TIMEOUT_SECS: u64 = 120;
const CONNECT_TIMEOUT_SECS: u64 = 10;

pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
}

impl AnthropicClient {
    pub fn new(api_key: String) -> Result<Self, AgentError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .build()
            .map_err(|e| AgentError::ApiRequest(e.to_string()))?;
        Ok(Self { http, api_key })
    }

    pub async fn chat(
        &self,
        model: &str,
        max_tokens: u32,
        system: &str,
        messages: &[Message],
        tools: Option<&[Tool]>,
    ) -> Result<ChatResponse, AgentError> {
        let body = ApiRequest {
            model,
            max_tokens,
            system,
            messages,
            tools,
        };

        let response = self
            .http
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
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
            return Err(AgentError::ApiResponse {
                status,
                body: text,
            });
        }

        parse_response(&text)
    }
}

#[derive(serde::Serialize)]
struct ApiRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: &'a [Message],
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [Tool]>,
}

#[derive(serde::Deserialize)]
struct ApiResponse {
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: String,
    usage: Usage,
}

#[derive(serde::Deserialize)]
struct Usage {
    input_tokens: u64,
    output_tokens: u64,
}

/// A model entry from the Anthropic Models API.
#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub id: String,
    pub display_name: String,
}

/// Hardcoded fallback models if the API call fails.
const FALLBACK_MODELS: &[(&str, &str)] = &[
    ("claude-opus-4-6", "Claude Opus 4.6"),
    ("claude-sonnet-4-6", "Claude Sonnet 4.6"),
    ("claude-haiku-4-5-20251001", "Claude Haiku 4.5"),
    ("claude-sonnet-4-5-20250929", "Claude Sonnet 4.5"),
    ("claude-opus-4-5-20251101", "Claude Opus 4.5"),
    ("claude-opus-4-1-20250805", "Claude Opus 4.1"),
    ("claude-sonnet-4-20250514", "Claude Sonnet 4"),
    ("claude-opus-4-20250514", "Claude Opus 4"),
];

#[derive(serde::Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

#[derive(serde::Deserialize)]
struct ModelInfo {
    id: String,
    display_name: String,
}

/// Fetch available models from the Anthropic API, with hardcoded fallback.
pub async fn fetch_models(api_key: &str) -> Vec<ModelEntry> {
    match fetch_models_from_api(api_key).await {
        Ok(models) if !models.is_empty() => models,
        Ok(_) => {
            warn!("Models API returned empty list, using fallback");
            fallback_models()
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch models from API, using fallback");
            fallback_models()
        }
    }
}

async fn fetch_models_from_api(api_key: &str) -> Result<Vec<ModelEntry>, AgentError> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| AgentError::ApiRequest(e.to_string()))?;

    let mut all_models = Vec::new();
    let mut after_id: Option<String> = None;

    // Paginate through all models (limit=1000 per page)
    loop {
        let mut req = http
            .get(MODELS_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", API_VERSION)
            .query(&[("limit", "1000")]);

        if let Some(ref cursor) = after_id {
            req = req.query(&[("after_id", cursor.as_str())]);
        }

        let response = req
            .send()
            .await
            .map_err(|e| AgentError::ApiRequest(e.to_string()))?;

        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|e| AgentError::ApiRequest(e.to_string()))?;

        if status != 200 {
            return Err(AgentError::ApiResponse {
                status,
                body: text,
            });
        }

        let resp: ModelsResponse =
            serde_json::from_str(&text).map_err(|e| AgentError::ApiParse(e.to_string()))?;

        let has_more = resp.data.len() == 1000;
        let last_id = resp.data.last().map(|m| m.id.clone());

        for m in resp.data {
            all_models.push(ModelEntry {
                id: m.id,
                display_name: m.display_name,
            });
        }

        if has_more {
            after_id = last_id;
        } else {
            break;
        }
    }

    Ok(all_models)
}

fn fallback_models() -> Vec<ModelEntry> {
    FALLBACK_MODELS
        .iter()
        .map(|(id, name)| ModelEntry {
            id: id.to_string(),
            display_name: name.to_string(),
        })
        .collect()
}

fn parse_response(json: &str) -> Result<ChatResponse, AgentError> {
    let api: ApiResponse =
        serde_json::from_str(json).map_err(|e| AgentError::ApiParse(e.to_string()))?;

    let content: Vec<ContentBlock> = api
        .content
        .into_iter()
        .filter(|block| !matches!(block, ContentBlock::Unknown))
        .collect();

    Ok(ChatResponse {
        content,
        model: api.model,
        stop_reason: api.stop_reason,
        input_tokens: api.usage.input_tokens,
        output_tokens: api.usage.output_tokens,
    })
}
