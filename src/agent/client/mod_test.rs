use super::{
    Adapter, Provider, ProviderConfig, create_client, default_model_for_provider, provider_from_id,
};
use crate::agent::types::AgentError;

#[test]
fn provider_ids_and_labels_are_stable() {
    assert_eq!(Provider::Anthropic.id(), "anthropic");
    assert_eq!(Provider::OpenRouter.id(), "openrouter");
    assert_eq!(Provider::OpenAI.id(), "openai");

    assert_eq!(Provider::Anthropic.label(), "Anthropic");
    assert_eq!(Provider::OpenRouter.label(), "OpenRouter");
    assert_eq!(Provider::OpenAI.label(), "OpenAI");
}

#[test]
fn adapter_parse_supports_aliases() {
    assert_eq!(
        Adapter::parse("anthropic"),
        Some(Adapter::AnthropicMessages)
    );
    assert_eq!(
        Adapter::parse("chat_completions"),
        Some(Adapter::OpenAIChatCompletions)
    );
    assert_eq!(Adapter::parse("chat"), Some(Adapter::OpenAIChatCompletions));
    assert_eq!(Adapter::parse("messages"), Some(Adapter::OpenAIMessages));
    assert_eq!(Adapter::parse("invalid"), None);
}

#[test]
fn provider_from_id_maps_known_providers() {
    assert_eq!(provider_from_id("anthropic"), Some(Provider::Anthropic));
    assert_eq!(provider_from_id("openrouter"), Some(Provider::OpenRouter));
    assert_eq!(provider_from_id("openai"), Some(Provider::OpenAI));
    assert_eq!(provider_from_id("unknown"), None);
}

#[test]
fn default_models_are_set_per_provider() {
    assert_eq!(default_model_for_provider(Provider::Anthropic), "claude-sonnet-4-6");
    assert_eq!(
        default_model_for_provider(Provider::OpenRouter),
        "openai/gpt-4o-mini"
    );
    assert_eq!(default_model_for_provider(Provider::OpenAI), "gpt-4o-mini");
}

#[test]
fn create_client_rejects_invalid_provider_adapter_pairings() {
    let anthropic_with_openai_adapter = ProviderConfig {
        provider: Provider::Anthropic,
        adapter: Adapter::OpenAIChatCompletions,
        api_key: "test".to_string(),
    };
    let err = match create_client(&anthropic_with_openai_adapter) {
        Ok(_) => panic!("expected an error"),
        Err(err) => err,
    };
    assert!(matches!(err, AgentError::ApiRequest(_)));

    let openai_with_anthropic_adapter = ProviderConfig {
        provider: Provider::OpenAI,
        adapter: Adapter::AnthropicMessages,
        api_key: "test".to_string(),
    };
    let err = match create_client(&openai_with_anthropic_adapter) {
        Ok(_) => panic!("expected an error"),
        Err(err) => err,
    };
    assert!(matches!(err, AgentError::ApiRequest(_)));
}
