pub mod client;
pub mod tools;
pub mod types;

use std::time::Instant;

use tokio::sync::mpsc;
use tracing::{info, warn};

use self::client::LlmClient;
use self::types::{
    AgentError, AgentRunResult, AgentStepRecord, ConfidenceLabel, Content, ContentBlock, Message,
    ToolCallRecord, VerificationCheck, VerificationReport,
};
use crate::api::GhostfolioClient;
use crate::config::Config;
use crate::langsmith::{LangSmithConfig, Trace};
use crate::text::truncate_utf8;
use crate::tools::ToolDispatcher;

const MAX_TOOL_ROUNDS: usize = 20;
const MAX_TOKENS: u32 = 4096;
const SYSTEM_PROMPT: &str = include_str!("system.md");

/// Event sent from the agent task to the TUI.
#[derive(Debug)]
pub enum AgentEvent {
    ToolCall(ToolCallRecord),
    ChartData(serde_json::Value),
    Response {
        text: String,
        input_tokens: u64,
        output_tokens: u64,
        last_input_tokens: u64,
        steps: usize,
        verified: bool,
        confidence_label: ConfidenceLabel,
        confidence_score: f32,
    },
    Error(String),
}

/// Run the agent ReAct loop in a background task.
pub fn spawn_agent(
    api_client: GhostfolioClient,
    llm_client: LlmClient,
    model: String,
    history: Vec<Message>,
    langsmith: Option<LangSmithConfig>,
    tx: mpsc::UnboundedSender<AgentEvent>,
) {
    tokio::spawn(async move {
        let dispatcher = ToolDispatcher::Live(api_client);
        match run_with_dispatcher(
            &llm_client,
            &model,
            history,
            &dispatcher,
            langsmith.as_ref(),
            None,
        )
        .await
        {
            Ok(result) => {
                for call in result.tool_calls {
                    let _ = tx.send(AgentEvent::ToolCall(call));
                }
                for chart in result.chart_data {
                    let _ = tx.send(AgentEvent::ChartData(chart));
                }
                let _ = tx.send(AgentEvent::Response {
                    text: result.text,
                    input_tokens: result.input_tokens,
                    output_tokens: result.output_tokens,
                    last_input_tokens: result.last_input_tokens,
                    steps: result.steps.len(),
                    verified: result.verified,
                    confidence_label: result.verification.confidence_label,
                    confidence_score: result.verification.confidence_score,
                });
            }
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(e.to_string()));
            }
        }
    });
}

pub async fn run_with_dispatcher(
    llm_client: &LlmClient,
    model: &str,
    mut messages: Vec<Message>,
    dispatcher: &ToolDispatcher,
    langsmith: Option<&LangSmithConfig>,
    tool_progress: Option<&mpsc::UnboundedSender<(String, bool)>>,
) -> Result<AgentRunResult, AgentError> {
    let tools = tools::all_tools();

    // Extract user input for the trace
    let user_input = extract_last_user_input(&messages);

    // Start LangSmith trace if configured
    let trace = langsmith.map(|cfg| Trace::start(cfg, model, &user_input));

    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    #[allow(unused_assignments)]
    let mut last_input_tokens: u64 = 0;
    let mut all_tool_calls: Vec<ToolCallRecord> = Vec::new();
    let mut steps_out: Vec<AgentStepRecord> = Vec::new();
    let mut chart_data: Vec<serde_json::Value> = Vec::new();

    for (steps, round) in (0..MAX_TOOL_ROUNDS).enumerate() {
        info!(
            round = round + 1,
            messages = messages.len(),
            "agent: llm round start"
        );

        let step_start = Instant::now();
        let response = llm_client
            .chat(model, MAX_TOKENS, SYSTEM_PROMPT, &messages, Some(&tools))
            .await?;
        let llm_duration_ms = step_start.elapsed().as_millis() as u64;

        total_input_tokens += response.input_tokens;
        total_output_tokens += response.output_tokens;
        last_input_tokens = response.input_tokens;

        // Log LLM call to LangSmith
        if let Some(ref t) = trace {
            t.log_llm_call(
                model,
                response.input_tokens,
                response.output_tokens,
                llm_duration_ms,
                &response.stop_reason,
            );
        }

        info!(
            round = round + 1,
            stop_reason = %response.stop_reason,
            blocks = response.content.len(),
            "agent: llm round result"
        );

        if response.stop_reason == "end_turn" || response.stop_reason == "max_tokens" {
            let text = extract_text(&response.content);

            // If the agent ended without any text (e.g. chart-only turn), nudge it
            // once to produce a summary. Only nudge on end_turn, not max_tokens.
            if text.trim().is_empty() && response.stop_reason == "end_turn" {
                messages.push(Message {
                    role: "assistant".to_string(),
                    content: Content::Blocks(response.content.clone()),
                });
                messages.push(Message {
                    role: "user".to_string(),
                    content: Content::Text(
                        "Please provide a text summary of the results above.".to_string(),
                    ),
                });
                continue;
            }

            steps_out.push(AgentStepRecord {
                step_number: steps + 1,
                duration_ms: llm_duration_ms,
                input_tokens: response.input_tokens,
                output_tokens: response.output_tokens,
                tool_calls: Vec::new(),
            });

            if let Some(ref t) = trace {
                t.finish(&text, total_input_tokens, total_output_tokens, steps + 1);
            }

            let tool_calls = all_tool_calls;
            let verification =
                verify_response(&user_input, &text, &tool_calls, llm_client, model).await;
            let verified = verification.verified;
            return Ok(AgentRunResult {
                text,
                input_tokens: total_input_tokens,
                output_tokens: total_output_tokens,
                last_input_tokens,
                steps: steps_out,
                tool_calls,
                chart_data,
                verified,
                verification,
            });
        }

        if response.stop_reason == "tool_use" {
            // Append assistant message with tool_use blocks
            messages.push(Message {
                role: "assistant".to_string(),
                content: Content::Blocks(response.content.clone()),
            });

            // Dispatch each tool and collect results
            let mut tool_results = Vec::new();
            let mut step_tool_calls = Vec::new();
            for block in &response.content {
                let ContentBlock::ToolUse { id, name, input } = block else {
                    continue;
                };

                let start = Instant::now();
                let result = dispatcher.dispatch(name, input).await;
                let duration_ms = start.elapsed().as_millis() as u64;

                let (content, is_error) = match result {
                    Ok(data) => {
                        // Emit chart data for rendering in the TUI
                        if data.get("chart_type").is_some() {
                            chart_data.push(data.clone());
                        }

                        // Truncate large responses to avoid context bloat
                        let s = data.to_string();
                        if s.len() > 4000 {
                            (format!("{}... (truncated)", truncate_utf8(&s, 4000)), false)
                        } else {
                            (s, false)
                        }
                    }
                    Err(e) => (format!("error: {e}"), true),
                };

                // Log tool call to LangSmith
                if let Some(ref t) = trace {
                    t.log_tool_call(name, duration_ms, !is_error);
                }

                let record = ToolCallRecord {
                    name: name.clone(),
                    duration_ms,
                    success: !is_error,
                };
                if let Some(tx) = tool_progress {
                    let _ = tx.send((name.clone(), !is_error));
                }

                step_tool_calls.push(record.clone());
                all_tool_calls.push(record);

                tool_results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content,
                    is_error: Some(is_error),
                });
            }

            // Append tool results as user message
            messages.push(Message {
                role: "user".to_string(),
                content: Content::Blocks(tool_results),
            });

            steps_out.push(AgentStepRecord {
                step_number: steps + 1,
                duration_ms: llm_duration_ms,
                input_tokens: response.input_tokens,
                output_tokens: response.output_tokens,
                tool_calls: step_tool_calls,
            });

            // Prune tool results from earlier rounds to save context
            // Keep only the latest tool result message (the one we just pushed) intact
            let last_idx = messages.len() - 1;
            for (idx, msg) in messages.iter_mut().enumerate() {
                if idx >= last_idx {
                    break;
                }
                if let Content::Blocks(ref mut blocks) = msg.content {
                    for block in blocks.iter_mut() {
                        if let ContentBlock::ToolResult { content, .. } = block {
                            if content.len() > 100 {
                                *content = format!("{}...", truncate_utf8(content, 100));
                            }
                        }
                    }
                }
            }

            continue;
        }

        warn!(stop_reason = %response.stop_reason, "agent: unknown stop_reason");

        if let Some(ref t) = trace {
            t.finish_error(&format!("Unexpected stop reason: {}", response.stop_reason));
        }

        return Err(AgentError::ApiRequest(format!(
            "Unexpected stop reason: {}",
            response.stop_reason
        )));
    }

    if let Some(ref t) = trace {
        t.finish_error(&format!("Max rounds exceeded: {}", MAX_TOOL_ROUNDS));
    }

    Err(AgentError::MaxRounds(MAX_TOOL_ROUNDS))
}

fn extract_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn extract_last_user_input(messages: &[Message]) -> String {
    messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| match &m.content {
            Content::Text(t) => t.clone(),
            Content::Blocks(_) => "(tool results)".to_string(),
        })
        .unwrap_or_default()
}

async fn verify_response(
    user_input: &str,
    response_text: &str,
    tool_calls: &[ToolCallRecord],
    llm_client: &LlmClient,
    active_model: &str,
) -> VerificationReport {
    let claim_to_tool_grounding =
        check_claim_to_tool_grounding(user_input, response_text, tool_calls);
    let tool_error_propagation = check_tool_error_propagation(response_text, tool_calls);
    let mut confidence_score = 1.0f32;

    if !claim_to_tool_grounding.pass {
        confidence_score -= 0.35;
    }
    if !tool_error_propagation.pass {
        confidence_score -= 0.30;
    }
    if looks_ambiguous(user_input) {
        confidence_score -= 0.15;
    }
    if tool_calls.len() > 2 {
        confidence_score -= 0.10;
    }
    confidence_score = confidence_score.clamp(0.0, 1.0);
    let mut confidence_label = score_to_label(confidence_score);

    let mut secondary_review = None;
    if should_run_secondary_review(
        user_input,
        response_text,
        tool_calls,
        confidence_score,
        &claim_to_tool_grounding,
        &tool_error_propagation,
    ) && let Some(verdict) = run_secondary_review(
        user_input,
        response_text,
        tool_calls,
        llm_client,
        active_model,
    )
    .await
    {
        // Merge secondary verdict into final status.
        if !verdict.pass {
            confidence_score = confidence_score.min(0.49);
        }
        secondary_review = Some(VerificationCheck {
            pass: verdict.pass,
            issues: verdict.issues,
        });
        confidence_label = score_to_label(confidence_score);
    }

    let verified = claim_to_tool_grounding.pass
        && tool_error_propagation.pass
        && secondary_review.as_ref().map(|c| c.pass).unwrap_or(true);

    VerificationReport {
        verified,
        confidence_label,
        confidence_score,
        claim_to_tool_grounding,
        tool_error_propagation,
        secondary_review,
    }
}

fn check_claim_to_tool_grounding(
    user_input: &str,
    response_text: &str,
    tool_calls: &[ToolCallRecord],
) -> VerificationCheck {
    let mut issues = Vec::new();
    if response_text.trim().is_empty() {
        issues.push("empty response".to_string());
    }

    let risky_claim = has_numeric_financial_claim(response_text)
        || has_money_terms(user_input)
        || has_money_terms(response_text);
    if !risky_claim {
        return VerificationCheck {
            pass: issues.is_empty(),
            issues,
        };
    }

    let successful_tools: std::collections::HashSet<&str> = tool_calls
        .iter()
        .filter(|tc| tc.success)
        .map(|tc| tc.name.as_str())
        .collect();

    let mut requires_any = vec![
        "get_portfolio_summary",
        "get_holdings",
        "get_holding_detail",
        "get_asset_profile",
        "price_history",
        "get_performance",
        "get_account_balances",
        "get_dividends",
        "get_investments",
        "get_benchmarks",
        "exchange_rate",
    ];
    if mentions_fx(user_input) || mentions_fx(response_text) {
        requires_any.push("exchange_rate");
    }
    if mentions_symbol_lookup(user_input) || mentions_symbol_lookup(response_text) {
        requires_any.push("search_assets");
    }
    if mentions_chart(response_text) {
        requires_any.push("chart_sparkline");
        requires_any.push("chart_bar");
    }

    if !requires_any
        .iter()
        .any(|name| successful_tools.contains(name))
    {
        issues.push(
            "response contains grounded financial claims without supporting successful tool calls"
                .to_string(),
        );
    }

    VerificationCheck {
        pass: issues.is_empty(),
        issues,
    }
}

fn check_tool_error_propagation(
    response_text: &str,
    tool_calls: &[ToolCallRecord],
) -> VerificationCheck {
    let mut issues = Vec::new();
    let had_error = tool_calls.iter().any(|tc| !tc.success);
    if !had_error {
        return VerificationCheck { pass: true, issues };
    }

    let lower = response_text.to_lowercase();
    let has_caveat = [
        "couldn't",
        "unable",
        "not enough data",
        "may be incomplete",
        "might be incomplete",
        "failed",
        "error",
        "cannot verify",
    ]
    .iter()
    .any(|kw| lower.contains(kw));

    if has_numeric_financial_claim(response_text) && !has_caveat {
        issues.push(
            "tool errors occurred, but response presents definitive numeric claims without caveats"
                .to_string(),
        );
    }

    VerificationCheck {
        pass: issues.is_empty(),
        issues,
    }
}

fn should_run_secondary_review(
    user_input: &str,
    response_text: &str,
    tool_calls: &[ToolCallRecord],
    confidence_score: f32,
    claim_to_tool_grounding: &VerificationCheck,
    tool_error_propagation: &VerificationCheck,
) -> bool {
    has_numeric_financial_claim(response_text)
        || mentions_fx(user_input)
        || confidence_score < 0.85
        || !claim_to_tool_grounding.pass
        || !tool_error_propagation.pass
        || tool_calls.iter().any(|tc| !tc.success)
        || tool_calls.len() > 2
}

#[derive(Debug, serde::Deserialize)]
struct SecondaryReviewVerdict {
    pass: bool,
    #[allow(dead_code)]
    confidence: Option<String>,
    #[serde(default)]
    issues: Vec<String>,
}

async fn run_secondary_review(
    user_input: &str,
    response_text: &str,
    tool_calls: &[ToolCallRecord],
    llm_client: &LlmClient,
    active_model: &str,
) -> Option<SecondaryReviewVerdict> {
    let cfg = Config::load();
    let verifier_provider = std::env::var("GF_VERIFY_PROVIDER")
        .ok()
        .and_then(|p| crate::agent::client::provider_from_id(p.trim()));
    let verifier_model = std::env::var("GF_VERIFY_MODEL").ok();
    let verifier_enabled = verifier_provider.is_some() || verifier_model.is_some();
    if !verifier_enabled {
        return None;
    }

    let (client, model) = if let Some(provider) = verifier_provider {
        let configured = cfg.configured_llm_providers();
        let provider_cfg = configured.into_iter().find(|p| p.provider == provider)?;
        let client = crate::agent::client::create_client(&provider_cfg).ok()?;
        let model = verifier_model.unwrap_or_else(|| cfg.model_for_provider(provider));
        (client, model)
    } else {
        // If only model override is set, reuse current client with a different model id.
        let model = verifier_model.unwrap_or_else(|| active_model.to_string());
        (llm_client.clone(), model)
    };

    let review_system = "You are a strict financial response verifier. Return ONLY valid JSON with fields: pass (bool), confidence (high|medium|low), issues (string[]). Fail if financial numeric claims are not supported by successful tool calls or if tool failures are ignored.";
    let review_input = serde_json::json!({
        "user_query": user_input,
        "assistant_response": response_text,
        "tool_calls": tool_calls,
        "constraints": [
            "Reject unsupported numeric claims",
            "Reject definitive answers after tool errors unless caveated"
        ]
    });
    let messages = vec![Message {
        role: "user".to_string(),
        content: Content::Text(review_input.to_string()),
    }];
    let resp = client
        .chat(&model, 512, review_system, &messages, None)
        .await
        .ok()?;
    let text = extract_text(&resp.content);
    parse_secondary_verdict(&text)
}

fn parse_secondary_verdict(text: &str) -> Option<SecondaryReviewVerdict> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<SecondaryReviewVerdict>(&text[start..=end]).ok()
}

fn score_to_label(score: f32) -> ConfidenceLabel {
    if score >= 0.8 {
        ConfidenceLabel::High
    } else if score >= 0.5 {
        ConfidenceLabel::Medium
    } else {
        ConfidenceLabel::Low
    }
}

fn has_numeric_financial_claim(text: &str) -> bool {
    text.contains('$')
        || text.contains('%')
        || text.chars().filter(|c| c.is_ascii_digit()).count() >= 3
        || mentions_ticker_like_token(text)
}

fn has_money_terms(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "price",
        "balance",
        "worth",
        "value",
        "convert",
        "rate",
        "portfolio",
        "holding",
        "benchmark",
    ]
    .iter()
    .any(|kw| lower.contains(kw))
}

fn mentions_fx(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "convert", "exchange", "fx", "usd", "eur", "jpy", "gbp", "cad", "aud",
    ]
    .iter()
    .any(|kw| lower.contains(kw))
}

fn mentions_symbol_lookup(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("ticker") || lower.contains("symbol")
}

fn mentions_chart(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("chart") || lower.contains("sparkline") || lower.contains("bar")
}

fn mentions_ticker_like_token(text: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|tok| (2..=5).contains(&tok.len()) && tok.chars().all(|c| c.is_ascii_uppercase()))
}

fn looks_ambiguous(user_input: &str) -> bool {
    let lower = user_input.to_lowercase();
    [
        "best", "compare", "should i", "maybe", "or", "and", "versus",
    ]
    .iter()
    .any(|kw| lower.contains(kw))
}
