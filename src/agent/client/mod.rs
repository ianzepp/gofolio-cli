pub mod anthropic;
pub mod openai;

use crate::agent::types::{AgentError, ChatResponse, Message, Tool};

/// A model entry returned by a provider's models API.
#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub id: String,
    pub display_name: String,
}

/// Known LLM providers.
#[derive(Debug, Clone)]
pub enum Provider {
    Anthropic,
    OpenRouter,
    OpenAI,
}

/// Unified LLM client — enum dispatch over exactly 2 implementations.
#[derive(Clone)]
pub enum LlmClient {
    Anthropic(anthropic::AnthropicClient),
    OpenAI(openai::OpenAIClient),
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
            Self::OpenAI(c) => c.chat(model, max_tokens, system, messages, tools).await,
        }
    }

    pub async fn fetch_models(&self) -> Vec<ModelEntry> {
        match self {
            Self::Anthropic(c) => c.fetch_models().await,
            Self::OpenAI(c) => c.fetch_models().await,
        }
    }

    pub fn provider_name(&self) -> &'static str {
        match self {
            Self::Anthropic(_) => "Anthropic",
            Self::OpenAI(_) => "OpenAI-compatible",
        }
    }
}

/// Create an LlmClient for the given provider.
pub fn create_client(provider: &Provider, api_key: String) -> Result<LlmClient, AgentError> {
    match provider {
        Provider::Anthropic => {
            let c = anthropic::AnthropicClient::new(api_key)?;
            Ok(LlmClient::Anthropic(c))
        }
        Provider::OpenRouter => {
            let c = openai::OpenAIClient::new(api_key, "https://openrouter.ai/api/v1".to_string())?;
            Ok(LlmClient::OpenAI(c))
        }
        Provider::OpenAI => {
            let c = openai::OpenAIClient::new(api_key, "https://api.openai.com/v1".to_string())?;
            Ok(LlmClient::OpenAI(c))
        }
    }
}
