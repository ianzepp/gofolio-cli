use std::collections::HashSet;
use std::fs::{create_dir_all, read_dir, read_to_string};
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::agent;
use crate::agent::client::{self, LlmClient, Provider, provider_from_id};
use crate::agent::types::Message;
use crate::config::Config;
use crate::tools::{MockFixtureSet, ToolDispatcher};

#[derive(Debug, Clone)]
pub struct TestArgs {
    pub suite: String,
    pub case_ids: Option<Vec<String>>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub evals_root: Option<PathBuf>,
    pub fixture_dir: Option<PathBuf>,
    pub live: bool,
    pub list_suites: bool,
}

#[derive(Debug, Deserialize)]
struct SuiteConfig {
    description: String,
    cases: SuiteCases,
    fixture: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SuiteCases {
    All(String),
    List(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct EvalCase {
    id: String,
    description: String,
    query: String,
    category: String,
    difficulty: String,
    expected_tools: Vec<String>,
    must_contain: Vec<String>,
    must_not_contain: Vec<String>,
    expected_verified: bool,
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
struct EvalResult {
    case_id: String,
    model: String,
    description: String,
    query: String,
    category: String,
    difficulty: String,
    tags: Vec<String>,
    pass: bool,
    tier_a: bool,
    tier_b: bool,
    tier_c: bool,
    detail_a: String,
    detail_b: String,
    detail_c: String,
    tools_called: Vec<String>,
    response: String,
    verified: bool,
    duration_ms: u64,
    input_tokens: u64,
    output_tokens: u64,
    timestamp: String,
    error: Option<String>,
    #[serde(skip_serializing)]
    steps: Vec<StepRun>,
}

#[derive(Debug)]
struct AgentCaseRun {
    response: String,
    tools_called: Vec<String>,
    steps: Vec<StepRun>,
    verified: bool,
    duration_ms: u64,
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
struct StepRun {
    step_number: usize,
    duration_ms: u64,
    tokens_in: u64,
    tokens_out: u64,
    tool_calls: Vec<ToolCallRun>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolCallRun {
    tool: String,
    #[serde(rename = "durationMs")]
    duration_ms: u64,
}

pub async fn run(args: TestArgs) -> Result<(), String> {
    let evals_root = resolve_evals_root(args.evals_root)?;
    let suites = load_suites(&evals_root)?;

    if args.list_suites {
        println!("Suites:");
        for (id, suite) in &suites {
            println!("- {id}: {}", suite.description);
        }
        return Ok(());
    }

    let suite = suites
        .get(&args.suite)
        .ok_or_else(|| format!("unknown suite '{}'", args.suite))?;
    let all_cases = load_cases(&evals_root)?;
    let mut cases = filter_cases(&all_cases, suite);
    if let Some(case_ids) = args.case_ids.as_ref() {
        let wanted: HashSet<&str> = case_ids.iter().map(String::as_str).collect();
        cases.retain(|c| wanted.contains(c.id.as_str()));
    }
    if cases.is_empty() {
        return Err("no cases selected".to_string());
    }

    let (llm_client, provider, model) = build_llm_client(args.model, args.provider)?;
    println!(
        "Running suite '{}' ({} cases) with provider={} model={}",
        args.suite,
        cases.len(),
        provider.label(),
        model
    );

    let run_started = Instant::now();
    let run_id = format!("rust-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    let dispatcher = if args.live {
        let cfg = Config::load();
        let (jwt, base_url) = crate::api::auth::authenticate(&cfg)
            .await
            .map_err(|e| format!("live auth failed: {e}"))?;
        ToolDispatcher::Live(crate::api::GhostfolioClient::new(base_url, jwt))
    } else {
        let fixture_dir = args.fixture_dir.unwrap_or_else(|| {
            let name = suite
                .fixture
                .clone()
                .unwrap_or_else(|| "moderate-portfolio".to_string());
            evals_root.join("fixtures").join(name)
        });
        let fixtures = MockFixtureSet::load_dir(&fixture_dir).map_err(|e| {
            format!(
                "failed to load fixtures from {}: {e}",
                fixture_dir.display()
            )
        })?;
        ToolDispatcher::Mock(fixtures)
    };

    let mut results = Vec::with_capacity(cases.len());
    for eval_case in &cases {
        let started = Instant::now();
        let result = run_case(&llm_client, &model, &dispatcher, eval_case).await;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        match result {
            Ok(run) => {
                let grade = grade_case(eval_case, &run);
                println!(
                    "[{}] {} :: {} ({elapsed_ms}ms)",
                    if grade.pass { "PASS" } else { "FAIL" },
                    eval_case.id,
                    eval_case.description
                );

                results.push(EvalResult {
                    case_id: eval_case.id.clone(),
                    model: model.clone(),
                    description: eval_case.description.clone(),
                    query: eval_case.query.clone(),
                    category: eval_case.category.clone(),
                    difficulty: eval_case.difficulty.clone(),
                    tags: eval_case.tags.clone(),
                    pass: grade.pass,
                    tier_a: grade.tier_a,
                    tier_b: grade.tier_b,
                    tier_c: grade.tier_c,
                    detail_a: grade.detail_a,
                    detail_b: grade.detail_b,
                    detail_c: grade.detail_c,
                    tools_called: run.tools_called,
                    response: run.response,
                    verified: run.verified,
                    duration_ms: run.duration_ms,
                    input_tokens: run.input_tokens,
                    output_tokens: run.output_tokens,
                    timestamp: Utc::now().to_rfc3339(),
                    error: None,
                    steps: run.steps,
                });
            }
            Err(e) => {
                println!("[ERROR] {} :: {} ({elapsed_ms}ms)", eval_case.id, e);
                results.push(EvalResult {
                    case_id: eval_case.id.clone(),
                    model: model.clone(),
                    description: eval_case.description.clone(),
                    query: eval_case.query.clone(),
                    category: eval_case.category.clone(),
                    difficulty: eval_case.difficulty.clone(),
                    tags: eval_case.tags.clone(),
                    pass: false,
                    tier_a: false,
                    tier_b: false,
                    tier_c: false,
                    detail_a: "N/A".to_string(),
                    detail_b: "N/A".to_string(),
                    detail_c: "N/A".to_string(),
                    tools_called: Vec::new(),
                    response: String::new(),
                    verified: false,
                    duration_ms: elapsed_ms,
                    input_tokens: 0,
                    output_tokens: 0,
                    timestamp: Utc::now().to_rfc3339(),
                    error: Some(e),
                    steps: Vec::new(),
                });
            }
        }
    }

    let total = results.len();
    let passed = results.iter().filter(|r| r.pass).count();
    let failed = total - passed;
    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let total_input_tokens: u64 = results.iter().map(|r| r.input_tokens).sum();
    let total_output_tokens: u64 = results.iter().map(|r| r.output_tokens).sum();
    let run_duration_ms = run_started.elapsed().as_millis() as u64;
    println!(
        "Summary: total={} passed={} failed={} errors={} input_tokens={} output_tokens={}",
        total, passed, failed, errors, total_input_tokens, total_output_tokens
    );

    write_results_jsonl(&evals_root, &results)?;
    write_results_sqlite(
        &evals_root,
        &run_id,
        &args.suite,
        &model,
        run_duration_ms,
        &results,
    )?;
    Ok(())
}

fn resolve_evals_root(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let candidates = [
        PathBuf::from("gauntlet/evals"),
        PathBuf::from("../gauntlet/evals"),
    ];
    for candidate in candidates {
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }
    Err("could not locate evals root (pass --evals-root)".to_string())
}

fn load_suites(root: &Path) -> Result<std::collections::HashMap<String, SuiteConfig>, String> {
    let yaml = read_to_string(root.join("suites.yaml")).map_err(|e| e.to_string())?;
    serde_yaml::from_str(&yaml).map_err(|e| format!("failed to parse suites.yaml: {e}"))
}

fn load_cases(root: &Path) -> Result<Vec<EvalCase>, String> {
    let mut cases = Vec::new();
    for folder in ["golden_sets", "scenarios"] {
        let dir = root.join(folder);
        if !dir.is_dir() {
            continue;
        }
        for entry in read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let yaml = read_to_string(&path).map_err(|e| e.to_string())?;
            let mut file_cases: Vec<EvalCase> = serde_yaml::from_str(&yaml)
                .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;
            cases.append(&mut file_cases);
        }
    }
    Ok(cases)
}

fn filter_cases<'a>(all_cases: &'a [EvalCase], suite: &SuiteConfig) -> Vec<&'a EvalCase> {
    match &suite.cases {
        SuiteCases::All(v) if v == "all" => all_cases.iter().collect(),
        SuiteCases::List(ids) => {
            let id_set: HashSet<&str> = ids.iter().map(String::as_str).collect();
            all_cases
                .iter()
                .filter(|case| id_set.contains(case.id.as_str()))
                .collect()
        }
        _ => Vec::new(),
    }
}

fn build_llm_client(
    override_model: Option<String>,
    override_provider: Option<String>,
) -> Result<(LlmClient, Provider, String), String> {
    let cfg = Config::load();
    let providers = cfg.configured_llm_providers();
    let provider = if let Some(provider_id) = override_provider {
        let parsed = provider_from_id(provider_id.trim().to_lowercase().as_str())
            .ok_or_else(|| format!("invalid provider '{}'", provider_id))?;
        if !providers.iter().any(|c| c.provider == parsed) {
            return Err(format!(
                "provider '{}' is not configured with an API key",
                parsed.id()
            ));
        }
        parsed
    } else if let Some(model) = override_model.as_ref() {
        // OpenRouter-style ids are "<provider>/<model>" and should route to OpenRouter client.
        if model.contains('/') && providers.iter().any(|c| c.provider == Provider::OpenRouter) {
            Provider::OpenRouter
        } else {
            cfg.preferred_llm_provider(&providers).ok_or_else(|| {
                "no LLM provider configured (set ANTHROPIC_API_KEY, OPENROUTER_API_KEY, or OPENAI_API_KEY)"
                    .to_string()
            })?
        }
    } else {
        cfg.preferred_llm_provider(&providers).ok_or_else(|| {
            "no LLM provider configured (set ANTHROPIC_API_KEY, OPENROUTER_API_KEY, or OPENAI_API_KEY)"
                .to_string()
        })?
    };
    let provider_cfg = providers
        .iter()
        .find(|c| c.provider == provider)
        .ok_or_else(|| "preferred provider was not configured".to_string())?;
    let client = client::create_client(provider_cfg).map_err(|e| e.to_string())?;
    let model = override_model.unwrap_or_else(|| cfg.model_for_provider(provider));
    Ok((client, provider, model))
}

async fn run_case(
    llm_client: &LlmClient,
    model: &str,
    dispatcher: &ToolDispatcher,
    eval_case: &EvalCase,
) -> Result<AgentCaseRun, String> {
    let messages = vec![Message {
        role: "user".to_string(),
        content: crate::agent::types::Content::Text(eval_case.query.clone()),
    }];
    let started = Instant::now();
    let result = agent::run_with_dispatcher(llm_client, model, messages, dispatcher, None)
        .await
        .map_err(|e| e.to_string())?;

    let steps = result
        .steps
        .into_iter()
        .map(|s| StepRun {
            step_number: s.step_number,
            duration_ms: s.duration_ms,
            tokens_in: s.input_tokens,
            tokens_out: s.output_tokens,
            tool_calls: s
                .tool_calls
                .into_iter()
                .map(|tc| ToolCallRun {
                    tool: tc.name,
                    duration_ms: tc.duration_ms,
                })
                .collect(),
        })
        .collect();

    Ok(AgentCaseRun {
        response: result.text,
        tools_called: result.tool_calls.into_iter().map(|tc| tc.name).collect(),
        steps,
        verified: result.verified,
        duration_ms: started.elapsed().as_millis() as u64,
        input_tokens: result.input_tokens,
        output_tokens: result.output_tokens,
    })
}

#[derive(Debug)]
struct Grade {
    pass: bool,
    tier_a: bool,
    tier_b: bool,
    tier_c: bool,
    detail_a: String,
    detail_b: String,
    detail_c: String,
}

fn grade_case(eval_case: &EvalCase, run: &AgentCaseRun) -> Grade {
    let (tier_a, detail_a) = grade_tier_a(eval_case, run);
    let (tier_b, detail_b) = grade_tier_b(eval_case, run);
    let (tier_c, detail_c) = grade_tier_c(eval_case, run);
    Grade {
        pass: tier_a && tier_b && tier_c,
        tier_a,
        tier_b,
        tier_c,
        detail_a,
        detail_b,
        detail_c,
    }
}

fn grade_tier_a(eval_case: &EvalCase, run: &AgentCaseRun) -> (bool, String) {
    let expected: HashSet<String> = eval_case
        .expected_tools
        .iter()
        .map(|t| normalize_tool_name(t))
        .collect();
    let actual: HashSet<String> = run
        .tools_called
        .iter()
        .map(|t| normalize_tool_name(t))
        .collect();

    if expected.is_empty() && actual.is_empty() {
        return (true, "No tools expected, none called".to_string());
    }

    let missing: Vec<String> = expected
        .iter()
        .filter(|tool| !actual.contains(*tool))
        .cloned()
        .collect();
    if missing.is_empty() {
        return (true, "All expected tools were called".to_string());
    }
    (
        false,
        format!(
            "Missing tools [{}]; called [{}]",
            missing.join(", "),
            actual.into_iter().collect::<Vec<_>>().join(", ")
        ),
    )
}

fn grade_tier_b(eval_case: &EvalCase, run: &AgentCaseRun) -> (bool, String) {
    let lower = run.response.to_lowercase();
    let missing: Vec<String> = eval_case
        .must_contain
        .iter()
        .filter(|s| !lower.contains(&s.to_lowercase()))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return (
            false,
            format!("Missing in response [{}]", missing.join(", ")),
        );
    }

    let forbidden: Vec<String> = eval_case
        .must_not_contain
        .iter()
        .filter(|s| lower.contains(&s.to_lowercase()))
        .cloned()
        .collect();
    if !forbidden.is_empty() {
        return (
            false,
            format!("Forbidden strings in response [{}]", forbidden.join(", ")),
        );
    }
    (true, "All content assertions passed".to_string())
}

fn grade_tier_c(eval_case: &EvalCase, run: &AgentCaseRun) -> (bool, String) {
    if run.verified == eval_case.expected_verified {
        return (true, format!("verified={} as expected", run.verified));
    }
    (
        false,
        format!(
            "Expected verified={}, got {}",
            eval_case.expected_verified, run.verified
        ),
    )
}

fn normalize_tool_name(name: &str) -> String {
    match name {
        "market_data" => "get_market_data".to_string(),
        "exchange_rate" => "calculate".to_string(),
        "account_overview" => "list_accounts".to_string(),
        "symbol_lookup" => "search_assets".to_string(),
        "asset_profile" => "get_asset_profile".to_string(),
        "benchmark_data" => "get_benchmarks".to_string(),
        "historical_data" => "get_performance".to_string(),
        "portfolio_summary" => "get_portfolio_summary".to_string(),
        "activity_history" => "list_activities".to_string(),
        _ => name.to_string(),
    }
}

fn write_results_jsonl(evals_root: &Path, results: &[EvalResult]) -> Result<(), String> {
    let results_dir = evals_root.join("results");
    create_dir_all(&results_dir).map_err(|e| e.to_string())?;
    let run_id = Utc::now().format("%Y%m%d-%H%M%S");
    let path = results_dir.join(format!("rust-run-{run_id}.jsonl"));
    let mut body = String::new();
    for result in results {
        body.push_str(
            &serde_json::to_string(result).map_err(|e| format!("serialize result failed: {e}"))?,
        );
        body.push('\n');
    }
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    println!("Wrote {}", path.display());
    Ok(())
}

fn write_results_sqlite(
    evals_root: &Path,
    run_id: &str,
    suite: &str,
    model: &str,
    duration_ms: u64,
    results: &[EvalResult],
) -> Result<(), String> {
    let results_dir = evals_root.join("results");
    create_dir_all(&results_dir).map_err(|e| e.to_string())?;
    let db_path = results_dir.join("results.db");

    let conn = Connection::open(&db_path)
        .map_err(|e| format!("failed to open sqlite db {}: {e}", db_path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("failed to set WAL mode: {e}"))?;

    conn.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS runs (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id      TEXT NOT NULL,
  suite       TEXT NOT NULL,
  model_id    TEXT NOT NULL,
  model_label TEXT NOT NULL,
  total       INTEGER NOT NULL,
  passed      INTEGER NOT NULL,
  failed      INTEGER NOT NULL,
  errors      INTEGER NOT NULL,
  duration_ms INTEGER NOT NULL,
  tokens_in   INTEGER NOT NULL,
  tokens_out  INTEGER NOT NULL,
  timestamp   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS results (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id       TEXT NOT NULL,
  model_id     TEXT NOT NULL,
  case_id      TEXT NOT NULL,
  suite        TEXT NOT NULL,
  pass         INTEGER NOT NULL,
  tier_a       INTEGER NOT NULL,
  tier_b       INTEGER NOT NULL,
  tier_c       INTEGER NOT NULL,
  tools_called TEXT NOT NULL,
  step_count   INTEGER NOT NULL,
  duration_ms  INTEGER NOT NULL,
  tokens_in    INTEGER NOT NULL,
  tokens_out   INTEGER NOT NULL,
  verified     INTEGER NOT NULL,
  error        TEXT,
  response     TEXT NOT NULL,
  timestamp    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS steps (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id       TEXT NOT NULL,
  model_id     TEXT NOT NULL,
  case_id      TEXT NOT NULL,
  step_number  INTEGER NOT NULL,
  duration_ms  INTEGER NOT NULL,
  tokens_in    INTEGER NOT NULL,
  tokens_out   INTEGER NOT NULL,
  tool_calls   TEXT NOT NULL
);
"#,
    )
    .map_err(|e| format!("failed to init sqlite schema: {e}"))?;

    let total = results.len() as i64;
    let passed = results.iter().filter(|r| r.pass).count() as i64;
    let failed = total - passed;
    let errors = results.iter().filter(|r| r.error.is_some()).count() as i64;
    let tokens_in: i64 = results.iter().map(|r| r.input_tokens as i64).sum();
    let tokens_out: i64 = results.iter().map(|r| r.output_tokens as i64).sum();
    let timestamp = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO runs (run_id, suite, model_id, model_label, total, passed, failed, errors, duration_ms, tokens_in, tokens_out, timestamp)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            run_id,
            suite,
            model,
            model,
            total,
            passed,
            failed,
            errors,
            duration_ms as i64,
            tokens_in,
            tokens_out,
            timestamp
        ],
    )
    .map_err(|e| format!("failed to insert sqlite run row: {e}"))?;

    for result in results {
        conn.execute(
            "INSERT INTO results (run_id, model_id, case_id, suite, pass, tier_a, tier_b, tier_c, tools_called, step_count, duration_ms, tokens_in, tokens_out, verified, error, response, timestamp)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                run_id,
                result.model,
                result.case_id,
                suite,
                if result.pass { 1 } else { 0 },
                if result.tier_a { 1 } else { 0 },
                if result.tier_b { 1 } else { 0 },
                if result.tier_c { 1 } else { 0 },
                serde_json::to_string(&result.tools_called)
                    .map_err(|e| format!("failed to encode tools_called: {e}"))?,
                result.steps.len() as i64,
                result.duration_ms as i64,
                result.input_tokens as i64,
                result.output_tokens as i64,
                if result.verified { 1 } else { 0 },
                result.error.clone(),
                result.response.clone(),
                result.timestamp.clone()
            ],
        )
        .map_err(|e| format!("failed to insert sqlite result row: {e}"))?;

        for step in &result.steps {
            conn.execute(
                "INSERT INTO steps (run_id, model_id, case_id, step_number, duration_ms, tokens_in, tokens_out, tool_calls)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    run_id,
                    result.model,
                    result.case_id,
                    step.step_number as i64,
                    step.duration_ms as i64,
                    step.tokens_in as i64,
                    step.tokens_out as i64,
                    serde_json::to_string(&step.tool_calls)
                        .map_err(|e| format!("failed to encode step tool_calls: {e}"))?
                ],
            )
            .map_err(|e| format!("failed to insert sqlite step row: {e}"))?;
        }
    }

    println!("Wrote {}", db_path.display());
    Ok(())
}
