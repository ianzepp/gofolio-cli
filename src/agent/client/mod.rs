pub mod anthropic;
pub mod openai;

use crate::agent::types::{AgentError, ChatResponse, Message, Tool};

/// A model entry returned by a provider's models API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub display_name: String,
}

/// Known LLM providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenRouter,
    OpenAI,
}

impl Provider {
    pub fn id(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenRouter => "openrouter",
            Self::OpenAI => "openai",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenRouter => "OpenRouter",
            Self::OpenAI => "OpenAI",
        }
    }
}

/// Request/response adapter used internally by a provider client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adapter {
    AnthropicMessages,
    OpenAIChatCompletions,
    OpenAIMessages,
}

impl Adapter {
    pub fn id(self) -> &'static str {
        match self {
            Self::AnthropicMessages => "anthropic_messages",
            Self::OpenAIChatCompletions => "openai_chat_completions",
            Self::OpenAIMessages => "openai_messages",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "anthropic_messages" | "anthropic" => Some(Self::AnthropicMessages),
            "openai_chat_completions" | "chat_completions" | "chat" => {
                Some(Self::OpenAIChatCompletions)
            }
            "openai_messages" | "messages" => Some(Self::OpenAIMessages),
            _ => None,
        }
    }
}

/// Provider configuration used to construct a client.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub provider: Provider,
    pub adapter: Adapter,
    pub api_key: String,
}

/// Unified LLM client — enum dispatch over provider+adapter implementations.
#[derive(Clone)]
pub enum LlmClient {
    Anthropic(anthropic::AnthropicClient),
    OpenAIChatCompletions(openai::OpenAIClient),
    OpenAIMessages(openai::OpenAIClient),
}

impl LlmClient {
    pub async fn chat(
        &self,
        model: &str,
        max_tokens: u32,
        system: &str,
        messages: &[Message],
        tools: Option<&[Tool]>,
    ) -> Result<ChatResponse, AgentError> {
        match self {
            Self::Anthropic(c) => c.chat(model, max_tokens, system, messages, tools).await,
            Self::OpenAIChatCompletions(c) => {
                c.chat_chat_completions(model, max_tokens, system, messages, tools)
                    .await
            }
            Self::OpenAIMessages(c) => {
                c.chat_messages(model, max_tokens, system, messages, tools)
                    .await
            }
        }
    }

    pub async fn fetch_models(&self) -> Vec<ModelEntry> {
        match self {
            Self::Anthropic(c) => c.fetch_models().await,
            Self::OpenAIChatCompletions(c) | Self::OpenAIMessages(c) => c.fetch_models().await,
        }
    }
}

/// Create an LlmClient for the given provider and adapter.
pub fn create_client(config: &ProviderConfig) -> Result<LlmClient, AgentError> {
    match (config.provider, config.adapter) {
        (Provider::Anthropic, Adapter::AnthropicMessages) => {
            let c = anthropic::AnthropicClient::new(config.api_key.clone())?;
            Ok(LlmClient::Anthropic(c))
        }
        (Provider::OpenRouter, Adapter::OpenAIChatCompletions)
        | (Provider::OpenAI, Adapter::OpenAIChatCompletions) => {
            let base_url = match config.provider {
                Provider::OpenRouter => "https://openrouter.ai/api/v1",
                Provider::OpenAI => "https://api.openai.com/v1",
                Provider::Anthropic => unreachable!(),
            };
            let c = openai::OpenAIClient::new(config.api_key.clone(), base_url.to_string())?;
            Ok(LlmClient::OpenAIChatCompletions(c))
        }
        (Provider::OpenRouter, Adapter::OpenAIMessages)
        | (Provider::OpenAI, Adapter::OpenAIMessages) => {
            let base_url = match config.provider {
                Provider::OpenRouter => "https://openrouter.ai/api/v1",
                Provider::OpenAI => "https://api.openai.com/v1",
                Provider::Anthropic => unreachable!(),
            };
            let c = openai::OpenAIClient::new(config.api_key.clone(), base_url.to_string())?;
            Ok(LlmClient::OpenAIMessages(c))
        }
        (Provider::Anthropic, _) => Err(AgentError::ApiRequest(
            "anthropic provider only supports anthropic_messages adapter".to_string(),
        )),
        (Provider::OpenRouter | Provider::OpenAI, Adapter::AnthropicMessages) => {
            Err(AgentError::ApiRequest(
                "openai-compatible providers do not support anthropic_messages adapter".to_string(),
            ))
        }
    }
}

pub fn default_model_for_provider(provider: Provider) -> &'static str {
    match provider {
        Provider::Anthropic => "claude-sonnet-4-6",
        Provider::OpenRouter => "openai/gpt-4o-mini",
        Provider::OpenAI => "gpt-4o-mini",
    }
}

pub fn provider_from_id(id: &str) -> Option<Provider> {
    match id {
        "anthropic" => Some(Provider::Anthropic),
        "openrouter" => Some(Provider::OpenRouter),
        "openai" => Some(Provider::OpenAI),
        _ => None,
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
