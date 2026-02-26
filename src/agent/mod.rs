pub mod client;
pub mod tools;
pub mod types;

use std::time::Instant;

use tokio::sync::mpsc;
use tracing::{info, warn};

use self::client::LlmClient;
use self::types::{
    AgentError, AgentRunResult, AgentStepRecord, Content, ContentBlock, Message, ToolCallRecord,
};
use crate::api::GhostfolioClient;
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
) -> Result<AgentRunResult, AgentError> {
    let tools = tools::all_tools();

    // Extract user input for the trace
    let user_input = extract_last_user_input(&messages);

    // Start LangSmith trace if configured
    let trace = langsmith.map(|cfg| Trace::start(cfg, model, &user_input));

    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
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

            return Ok(AgentRunResult {
                text,
                input_tokens: total_input_tokens,
                output_tokens: total_output_tokens,
                last_input_tokens,
                steps: steps_out,
                tool_calls: all_tool_calls,
                chart_data,
                verified: messages.iter().any(|m| {
                    matches!(
                        &m.content,
                        Content::Blocks(blocks)
                            if blocks
                                .iter()
                                .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
                    )
                }),
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
