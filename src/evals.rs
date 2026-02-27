use std::collections::{BTreeMap, HashSet};
use std::fs::{create_dir_all, read_dir, read_to_string};
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::agent;
use crate::agent::client::{self, Adapter, LlmClient, Provider, ProviderConfig, provider_from_id};
use crate::agent::types::{Message, VerificationReport};
use crate::config::Config;
use crate::evals_tui::TuiEvent;
use crate::key_pool::KeyPool;
use crate::langsmith::LangSmithConfig;
use crate::tools::{MockFixtureSet, ToolDispatcher};

#[derive(Debug, Clone)]
pub struct TestArgs {
    pub suite: String,
    pub case_ids: Option<Vec<String>>,
    pub model: Option<String>,
    pub models: Option<Vec<String>>,
    pub provider: Option<String>,
    pub evals_root: Option<PathBuf>,
    pub fixture_dir: Option<PathBuf>,
    pub live: bool,
    pub list_suites: bool,
    pub no_tui: bool,
}

#[derive(Debug, Clone)]
pub struct ReportArgs {
    pub evals_root: Option<PathBuf>,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GetArgs {
    pub path: PathBuf,
    pub case_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReplayArgs {
    pub evals_root: Option<PathBuf>,
    pub run_id: Option<String>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
struct StoredEvalResult {
    model: String,
    category: String,
    difficulty: String,
    pass: bool,
    error: Option<String>,
    duration_ms: u64,
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct ReplaySourceCase {
    case_id: String,
    model: String,
    pass: bool,
    #[serde(default)]
    tools_called: Vec<String>,
    #[serde(default)]
    response: String,
    #[serde(default)]
    verified: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    duration_ms: u64,
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
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

#[derive(Debug, Clone, Deserialize)]
struct EvalCase {
    id: String,
    description: String,
    query: String,
    category: String,
    difficulty: String,
    expected_tools: Vec<String>,
    #[serde(default)]
    tool_must_contain: Vec<String>,
    #[serde(default)]
    tool_must_not_contain: Vec<String>,
    must_contain: Vec<String>,
    #[serde(default)]
    must_contain_any: Vec<String>,
    must_not_contain: Vec<String>,
    expected_verified: bool,
    #[serde(default)]
    skip_verified: bool,
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
    verification: Option<VerificationReport>,
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
    verification: VerificationReport,
    duration_ms: u64,
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Clone)]
struct ModelTarget {
    client: LlmClient,
    provider: Provider,
    model: String,
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

#[derive(Debug, Serialize)]
struct RunSummaryFile {
    run_id: String,
    suite: String,
    total: usize,
    passed: usize,
    failed: usize,
    errors: usize,
    input_tokens: u64,
    output_tokens: u64,
    duration_ms: u64,
    latency_p50_ms: u64,
    latency_p95_ms: u64,
}

pub async fn run(args: TestArgs) -> Result<(), String> {
    let evals_root = resolve_evals_root(args.evals_root)?;
    let suites = load_suites(&evals_root)?;
    let runtime_cfg = Config::load();
    let langsmith = LangSmithConfig::from_config(&runtime_cfg);

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

    let model_overrides = normalize_model_overrides(args.model, args.models)?;
    let targets = build_model_targets(model_overrides, args.provider)?;
    let model_labels: Vec<String> = targets
        .iter()
        .map(|t| format!("{}:{}", t.provider.id(), t.model))
        .collect();
    println!(
        "Running suite '{}' ({} cases x {} models)",
        args.suite,
        cases.len(),
        targets.len()
    );
    println!("Models: {}", model_labels.join(", "));

    let run_started = Instant::now();
    let run_id = format!("rust-run-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    let dispatcher = if args.live {
        let (jwt, base_url) = crate::api::auth::authenticate(&runtime_cfg)
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

    let use_tui = !args.no_tui && std::io::IsTerminal::is_terminal(&std::io::stderr());

    let mut results = Vec::with_capacity(cases.len() * targets.len());

    for target in &targets {
        let keys = runtime_cfg.api_keys_for_provider(target.provider);
        if keys.is_empty() {
            return Err(format!(
                "no API keys found for provider '{}'",
                target.provider.id()
            ));
        }

        if use_tui {
            // TUI mode with key pooling (parallel across available keys)
            let key_pool = KeyPool::new(keys.clone());
            let pool_target = ModelTargetPool {
                provider: target.provider,
                adapter: provider_adapter(target.provider, &runtime_cfg),
                model: target.model.clone(),
            };
            let model_label = format!("{}:{}", target.provider.id(), target.model);

            let (tui_tx, tui_rx) = mpsc::unbounded_channel();

            let suite_name = args.suite.clone();
            let tui_run_id = run_id.clone();
            let total_cases = cases.len();
            let key_count = key_pool.key_count();
            let max_concurrent = key_pool.max_concurrent();
            let ml = model_label.clone();

            let cancel = tokio_util::sync::CancellationToken::new();

            let tui_cancel = cancel.clone();
            let tui_handle = tokio::spawn(async move {
                let result = crate::evals_tui::run_tui(
                    tui_rx,
                    &suite_name,
                    &tui_run_id,
                    total_cases,
                    key_count,
                    max_concurrent,
                    &ml,
                )
                .await;
                // If TUI exited early (user pressed q), cancel the workers
                if result.is_err() {
                    tui_cancel.cancel();
                }
                result
            });

            let worker_cancel = cancel.clone();
            let worker_run_dir = evals_root.join("results").join(&run_id);
            let worker_handle = tokio::spawn({
                let cases: Vec<EvalCase> = cases.iter().map(|c| (*c).clone()).collect();
                let pool_target = pool_target.clone();
                let dispatcher = dispatcher.clone();
                let langsmith = langsmith.clone();
                async move {
                    let case_refs: Vec<&EvalCase> = cases.iter().collect();
                    run_cases_parallel_tui(
                        &case_refs,
                        &pool_target,
                        &dispatcher,
                        &key_pool,
                        langsmith.as_ref(),
                        tui_tx,
                        &worker_cancel,
                        worker_run_dir,
                    )
                    .await
                }
            });

            // Workers run until done or cancelled; TUI cancels on `q`
            let mut model_results = worker_handle
                .await
                .map_err(|e| format!("worker join: {e}"))??;

            // TUI may already be done (AllDone received), or cancelled — either way wait
            let _ = tui_handle.await;

            results.append(&mut model_results);
        } else {
            // Plain-text serial mode (CI / piped output)
            println!(
                "Model run: provider={} model={}",
                target.provider.label(),
                target.model
            );
            let mut model_results = run_cases_serial(
                &cases,
                target,
                &dispatcher,
                langsmith.as_ref(),
                evals_root.join("results").join(&run_id),
            )
            .await?;
            for r in &model_results {
                let status = if r.pass { "PASS" } else { "FAIL" };
                let tools = r.tools_called.join(", ");
                println!("[{status}] {} — {}", r.case_id, r.description);
                if !r.pass {
                    if !r.detail_a.is_empty() {
                        println!("  tools:    {}", r.detail_a);
                    }
                    if !r.detail_b.is_empty() {
                        println!("  content:  {}", r.detail_b);
                    }
                    if !r.detail_c.is_empty() {
                        println!("  verified: {}", r.detail_c);
                    }
                    if !tools.is_empty() {
                        println!("  called:   {tools}");
                    }
                    if let Some(ref err) = r.error {
                        println!("  error:    {err}");
                    }
                    println!("  response: {}", r.response);
                }
            }
            results.append(&mut model_results);
        }
    }

    results.sort_by(|a, b| {
        a.model
            .cmp(&b.model)
            .then_with(|| a.case_id.cmp(&b.case_id))
    });

    let total = results.len();
    let passed = results.iter().filter(|r| r.pass).count();
    let failed = total - passed;
    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let total_input_tokens: u64 = results.iter().map(|r| r.input_tokens).sum();
    let total_output_tokens: u64 = results.iter().map(|r| r.output_tokens).sum();
    let run_duration_ms = run_started.elapsed().as_millis() as u64;
    let (latency_p50_ms, latency_p95_ms) =
        compute_latency_percentiles(results.iter().map(|r| r.duration_ms));
    println!(
        "Summary: total={} passed={} failed={} errors={} input_tokens={} output_tokens={} duration={} p50={} p95={}",
        total,
        passed,
        failed,
        errors,
        total_input_tokens,
        total_output_tokens,
        format_seconds(run_duration_ms),
        format_seconds(latency_p50_ms),
        format_seconds(latency_p95_ms)
    );
    print_model_summary(&results);
    if targets.len() > 1 {
        print_model_comparison(&results);
    }

    write_results_jsonl(&evals_root, &run_id, &results)?;
    write_summary_file(
        &evals_root,
        &run_id,
        &args.suite,
        total,
        passed,
        failed,
        errors,
        total_input_tokens,
        total_output_tokens,
        run_duration_ms,
        latency_p50_ms,
        latency_p95_ms,
    )?;
    write_results_sqlite(
        &evals_root,
        &run_id,
        &args.suite,
        &model_labels.join(", "),
        run_duration_ms,
        latency_p50_ms,
        latency_p95_ms,
        &results,
    )?;

    if failed > 0 || errors > 0 {
        return Err(format!(
            "eval suite failed: total={total} passed={passed} failed={failed} errors={errors}"
        ));
    }

    Ok(())
}

fn build_eval_result(
    eval_case: EvalCase,
    model: String,
    elapsed_ms: u64,
    result: Result<AgentCaseRun, String>,
) -> EvalResult {
    match result {
        Ok(run) => {
            let grade = grade_case(&eval_case, &run);
            println!(
                "[{}] {} :: {} ({})",
                if grade.pass { "PASS" } else { "FAIL" },
                eval_case.id,
                eval_case.description,
                format_seconds(elapsed_ms)
            );
            if !grade.pass {
                if !grade.tier_a {
                    println!("  fail tier_a: {}", grade.detail_a);
                }
                if !grade.tier_b {
                    println!("  fail tier_b: {}", grade.detail_b);
                }
                if !grade.tier_c {
                    println!("  fail tier_c: {}", grade.detail_c);
                }
            }
            print_case_trace(&run);
            EvalResult {
                case_id: eval_case.id,
                model,
                description: eval_case.description,
                query: eval_case.query,
                category: eval_case.category,
                difficulty: eval_case.difficulty,
                tags: eval_case.tags,
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
                verification: Some(run.verification.clone()),
                duration_ms: run.duration_ms,
                input_tokens: run.input_tokens,
                output_tokens: run.output_tokens,
                timestamp: Utc::now().to_rfc3339(),
                error: None,
                steps: run.steps,
            }
        }
        Err(e) => {
            println!("[ERROR] {} :: {} ({})", eval_case.id, e, format_seconds(elapsed_ms));
            EvalResult {
                case_id: eval_case.id,
                model,
                description: eval_case.description,
                query: eval_case.query,
                category: eval_case.category,
                difficulty: eval_case.difficulty,
                tags: eval_case.tags,
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
                verification: None,
                duration_ms: elapsed_ms,
                input_tokens: 0,
                output_tokens: 0,
                timestamp: Utc::now().to_rfc3339(),
                error: Some(e),
                steps: Vec::new(),
            }
        }
    }
}

fn print_case_trace(run: &AgentCaseRun) {
    if run.steps.is_empty() {
        println!("  steps: none");
    } else {
        for step in &run.steps {
            println!(
                "  step {}: {} | {}+{} tok",
                step.step_number,
                format_seconds(step.duration_ms),
                step.tokens_in,
                step.tokens_out
            );
            if step.tool_calls.is_empty() {
                println!("    tools: none");
            } else {
                for tool_call in &step.tool_calls {
                    println!(
                        "    tool {}: {}",
                        tool_call.tool,
                        format_seconds(tool_call.duration_ms)
                    );
                }
            }
        }
    }
    println!(
        "  total: {} | {}+{} tok | verified={} confidence={:.0}%",
        format_seconds(run.duration_ms),
        run.input_tokens,
        run.output_tokens,
        run.verified,
        run.verification.confidence_score * 100.0
    );
}

pub fn report(args: ReportArgs) -> Result<(), String> {
    let evals_root = resolve_evals_root(args.evals_root)?;
    let run_dir = resolve_run_dir(&evals_root, args.run_id.as_deref())?;
    let run_id = run_dir
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("(unknown)");
    let results = load_run_results(&run_dir)?;
    if results.is_empty() {
        return Err(format!(
            "no case result files found in {}",
            run_dir.display()
        ));
    }

    let total = results.len();
    let passed = results.iter().filter(|r| r.pass).count();
    let failed = total - passed;
    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let total_input_tokens: u64 = results.iter().map(|r| r.input_tokens).sum();
    let total_output_tokens: u64 = results.iter().map(|r| r.output_tokens).sum();
    let total_duration_ms: u64 = results.iter().map(|r| r.duration_ms).sum();
    let (latency_p50_ms, latency_p95_ms) =
        compute_latency_percentiles(results.iter().map(|r| r.duration_ms));

    println!("Run: {run_id}");
    println!("Path: {}", run_dir.display());
    println!(
        "Summary: total={} passed={} failed={} errors={} input_tokens={} output_tokens={} duration={} p50={} p95={}",
        total,
        passed,
        failed,
        errors,
        total_input_tokens,
        total_output_tokens,
        format_seconds(total_duration_ms),
        format_seconds(latency_p50_ms),
        format_seconds(latency_p95_ms)
    );

    let mut by_model: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new();
    let mut by_category: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut by_difficulty: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut matrix: BTreeMap<&str, BTreeMap<&str, (usize, usize)>> = BTreeMap::new();
    let mut difficulties: BTreeMap<&str, ()> = BTreeMap::new();

    for result in &results {
        let model_stats = by_model.entry(result.model.as_str()).or_insert((0, 0, 0));
        model_stats.0 += 1;
        if result.pass {
            model_stats.1 += 1;
        }
        if result.error.is_some() {
            model_stats.2 += 1;
        }

        let category_stats = by_category.entry(result.category.as_str()).or_insert((0, 0));
        category_stats.0 += 1;
        if result.pass {
            category_stats.1 += 1;
        }

        let difficulty_stats = by_difficulty
            .entry(result.difficulty.as_str())
            .or_insert((0, 0));
        difficulty_stats.0 += 1;
        if result.pass {
            difficulty_stats.1 += 1;
        }

        difficulties.entry(result.difficulty.as_str()).or_insert(());
        let category_row = matrix.entry(result.category.as_str()).or_default();
        let cell = category_row
            .entry(result.difficulty.as_str())
            .or_insert((0, 0));
        cell.0 += 1;
        if result.pass {
            cell.1 += 1;
        }
    }

    println!("By model:");
    for (model, (count, model_passed, model_errors)) in by_model {
        let pass_rate = if count > 0 {
            (model_passed as f64 / count as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "- {} total={} passed={} pass_rate={:.1}% errors={}",
            model, count, model_passed, pass_rate, model_errors
        );
    }

    println!("By category:");
    for (category, (count, category_passed)) in &by_category {
        let pass_rate = if *count > 0 {
            (*category_passed as f64 / *count as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "- {} total={} passed={} pass_rate={:.1}%",
            category, count, category_passed, pass_rate
        );
    }

    println!("By difficulty:");
    for (difficulty, (count, difficulty_passed)) in &by_difficulty {
        let pass_rate = if *count > 0 {
            (*difficulty_passed as f64 / *count as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "- {} total={} passed={} pass_rate={:.1}%",
            difficulty, count, difficulty_passed, pass_rate
        );
    }

    let difficulty_columns: Vec<&str> = difficulties.keys().copied().collect();
    println!("Category x Difficulty:");
    let header = if difficulty_columns.is_empty() {
        "category".to_string()
    } else {
        format!("category | {}", difficulty_columns.join(" | "))
    };
    println!("{header}");
    for (category, row) in matrix {
        let mut values = Vec::new();
        for difficulty in &difficulty_columns {
            let (count, category_passed) = row.get(difficulty).copied().unwrap_or((0, 0));
            if count == 0 {
                values.push("-".to_string());
            } else {
                values.push(format!("{}/{} ({:.1}%)", category_passed, count, (category_passed as f64 / count as f64) * 100.0));
            }
        }
        if values.is_empty() {
            println!("{category}");
        } else {
            println!("{} | {}", category, values.join(" | "));
        }
    }

    Ok(())
}

pub fn get(args: GetArgs) -> Result<(), String> {
    let path = args.path;
    if path.is_file() {
        let body = read_to_string(&path).map_err(|e| e.to_string())?;
        println!("{body}");
        return Ok(());
    }

    if !path.is_dir() {
        return Err(format!("path does not exist: {}", path.display()));
    }

    if let Some(case_id) = args.case_id {
        let case_path = path.join(format!("{case_id}.json"));
        if !case_path.is_file() {
            return Err(format!(
                "case result not found: {}",
                case_path.display()
            ));
        }
        let body = read_to_string(&case_path).map_err(|e| e.to_string())?;
        println!("{body}");
        return Ok(());
    }

    let mut files = Vec::new();
    for entry in read_dir(&path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_path = entry.path();
        if file_path.extension().and_then(|ext| ext.to_str()) == Some("json")
            && let Some(stem) = file_path.file_stem().and_then(|s| s.to_str())
        {
            files.push(stem.to_string());
        }
    }
    files.sort();

    println!("Run path: {}", path.display());
    println!("Cases: {}", files.len());
    for case_id in files {
        println!("- {case_id}");
    }

    Ok(())
}

fn print_model_summary(results: &[EvalResult]) {
    #[derive(Default)]
    struct Summary {
        total: usize,
        passed: usize,
        errors: usize,
        input_tokens: u64,
        output_tokens: u64,
        duration_ms: u64,
    }

    let mut per_model: BTreeMap<&str, Summary> = BTreeMap::new();
    for result in results {
        let summary = per_model.entry(result.model.as_str()).or_default();
        summary.total += 1;
        if result.pass {
            summary.passed += 1;
        }
        if result.error.is_some() {
            summary.errors += 1;
        }
        summary.input_tokens += result.input_tokens;
        summary.output_tokens += result.output_tokens;
        summary.duration_ms += result.duration_ms;
    }

    println!("Model summary:");
    for (model, summary) in per_model {
        println!(
            "- {} total={} passed={} failed={} errors={} input_tokens={} output_tokens={} duration={}",
            model,
            summary.total,
            summary.passed,
            summary.total - summary.passed,
            summary.errors,
            summary.input_tokens,
            summary.output_tokens,
            format_seconds(summary.duration_ms)
        );
    }
}

fn format_seconds(duration_ms: u64) -> String {
    format!("{:.2}s", duration_ms as f64 / 1000.0)
}

fn print_model_comparison(results: &[EvalResult]) {
    let mut by_case: BTreeMap<&str, Vec<&EvalResult>> = BTreeMap::new();
    for result in results {
        by_case
            .entry(result.case_id.as_str())
            .or_default()
            .push(result);
    }

    println!("Cross-model diffs:");
    let mut diff_count = 0usize;
    for (case_id, rows) in by_case {
        let first = rows.first().map(|r| r.pass).unwrap_or(false);
        let has_diff = rows.iter().any(|r| r.pass != first);
        if !has_diff {
            continue;
        }
        diff_count += 1;
        let statuses: Vec<String> = rows
            .iter()
            .map(|r| format!("{}={}", r.model, if r.pass { "PASS" } else { "FAIL" }))
            .collect();
        println!("- {} :: {}", case_id, statuses.join(", "));
    }
    if diff_count == 0 {
        println!("- none");
    }
}

fn resolve_evals_root(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let candidates = [PathBuf::from("evals"), PathBuf::from("cli/evals")];
    for candidate in candidates {
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }
    Err("could not locate evals root (pass --evals-root)".to_string())
}

fn resolve_run_dir(evals_root: &Path, run_id: Option<&str>) -> Result<PathBuf, String> {
    let results_root = evals_root.join("results");
    if !results_root.is_dir() {
        return Err(format!(
            "results directory not found: {}",
            results_root.display()
        ));
    }

    if let Some(id) = run_id {
        let run_dir = results_root.join(id);
        if run_dir.is_dir() {
            return Ok(run_dir);
        }
        return Err(format!("run not found: {}", run_dir.display()));
    }

    let mut run_dirs: Vec<PathBuf> = read_dir(&results_root)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .collect();
    run_dirs.sort();
    run_dirs
        .pop()
        .ok_or_else(|| format!("no runs found under {}", results_root.display()))
}

fn load_run_results(run_dir: &Path) -> Result<Vec<StoredEvalResult>, String> {
    let mut results = Vec::new();
    for entry in read_dir(run_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let body = read_to_string(&path).map_err(|e| e.to_string())?;
        let result: StoredEvalResult = serde_json::from_str(&body)
            .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;
        results.push(result);
    }
    Ok(results)
}

fn load_replay_source_cases(run_dir: &Path) -> Result<Vec<ReplaySourceCase>, String> {
    let mut cases = Vec::new();
    for entry in read_dir(run_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let body = read_to_string(&path).map_err(|e| e.to_string())?;
        let case: ReplaySourceCase = serde_json::from_str(&body)
            .map_err(|e| format!("failed to parse replay case {}: {e}", path.display()))?;
        cases.push(case);
    }
    cases.sort_by(|a, b| a.case_id.cmp(&b.case_id));
    Ok(cases)
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

fn normalize_model_overrides(
    model: Option<String>,
    models: Option<Vec<String>>,
) -> Result<Vec<String>, String> {
    let values = if let Some(list) = models {
        list
    } else if let Some(single) = model {
        vec![single]
    } else {
        Vec::new()
    };
    let mut out = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push(trimmed.to_string());
    }
    if out.is_empty() {
        return Ok(Vec::new());
    }
    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for model in out {
        if seen.insert(model.clone()) {
            deduped.push(model);
        }
    }
    Ok(deduped)
}

fn build_model_targets(
    override_models: Vec<String>,
    override_provider: Option<String>,
) -> Result<Vec<ModelTarget>, String> {
    let cfg = Config::load();
    let providers = cfg.configured_llm_providers();
    let forced_provider = if let Some(provider_id) = override_provider {
        let parsed = provider_from_id(provider_id.trim().to_lowercase().as_str())
            .ok_or_else(|| format!("invalid provider '{}'", provider_id))?;
        if !providers.iter().any(|c| c.provider == parsed) {
            return Err(format!(
                "provider '{}' is not configured with an API key",
                parsed.id()
            ));
        }
        Some(parsed)
    } else {
        None
    };

    let models = if override_models.is_empty() {
        vec![String::new()]
    } else {
        override_models
    };

    let mut targets = Vec::new();
    for override_model in models {
        let provider = if let Some(p) = forced_provider {
            p
        } else if !override_model.is_empty()
            && override_model.contains('/')
            && providers.iter().any(|c| c.provider == Provider::OpenRouter)
        {
            // OpenRouter-style ids are "<provider>/<model>" and should route to OpenRouter client.
            Provider::OpenRouter
        } else {
            cfg.preferred_llm_provider(&providers).ok_or_else(|| {
                "no LLM provider configured (set ANTHROPIC_API_KEY, OPENROUTER_API_KEY, or OPENAI_API_KEY)"
                    .to_string()
            })?
        };

        let provider_cfg = providers
            .iter()
            .find(|c| c.provider == provider)
            .ok_or_else(|| format!("provider '{}' is not configured", provider.id()))?;
        let client = client::create_client(provider_cfg).map_err(|e| e.to_string())?;
        let model = if override_model.is_empty() {
            cfg.model_for_provider(provider)
        } else {
            override_model
        };
        targets.push(ModelTarget {
            client,
            provider,
            model,
        });
    }
    Ok(targets)
}

/// Plain-text serial execution (for CI / piped output / --no-tui).
async fn run_cases_serial(
    cases: &[&EvalCase],
    target: &ModelTarget,
    dispatcher: &ToolDispatcher,
    langsmith: Option<&LangSmithConfig>,
    run_dir: PathBuf,
) -> Result<Vec<EvalResult>, String> {
    create_dir_all(&run_dir).map_err(|e| e.to_string())?;
    let mut results = Vec::with_capacity(cases.len());
    for eval_case in cases {
        let started = Instant::now();
        let result = run_case(
            &target.client,
            &target.model,
            dispatcher,
            eval_case,
            langsmith,
        )
        .await;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let eval_result = build_eval_result(
            (*eval_case).clone(),
            target.model.clone(),
            elapsed_ms,
            result,
        );
        write_result_file(&run_dir, &eval_result);
        results.push(eval_result);
    }
    Ok(results)
}

/// Like [`ModelTarget`] but without a pre-built client — client is created
/// per-lease from the key pool.
#[derive(Clone)]
struct ModelTargetPool {
    provider: Provider,
    adapter: Adapter,
    model: String,
}

async fn run_cases_parallel_tui(
    cases: &[&EvalCase],
    target: &ModelTargetPool,
    dispatcher: &ToolDispatcher,
    key_pool: &KeyPool,
    langsmith: Option<&LangSmithConfig>,
    tui_tx: mpsc::UnboundedSender<TuiEvent>,
    cancel: &tokio_util::sync::CancellationToken,
    run_dir: PathBuf,
) -> Result<Vec<EvalResult>, String> {
    create_dir_all(&run_dir).map_err(|e| e.to_string())?;
    let mut results = Vec::with_capacity(cases.len());
    let mut join_set = JoinSet::new();
    let mut index = 0usize;

    while index < cases.len() || !join_set.is_empty() {
        // Check for cancellation
        if cancel.is_cancelled() {
            join_set.abort_all();
            break;
        }

        // Fill up to pool_size concurrent tasks
        while join_set.len() < key_pool.max_concurrent() && index < cases.len() {
            let eval_case = (*cases[index]).clone();
            index += 1;
            let model = target.model.clone();
            let provider = target.provider;
            let adapter = target.adapter;
            let dispatcher = dispatcher.clone();
            let langsmith = langsmith.cloned();
            let tx = tui_tx.clone();

            // Lease a key — blocks if all keys are in use; bail on cancel
            let lease = tokio::select! {
                l = key_pool.lease() => l,
                _ = cancel.cancelled() => break,
            };

            join_set.spawn(async move {
                let _ = tx.send(TuiEvent::CaseStarted {
                    case_id: eval_case.id.clone(),
                    description: eval_case.description.clone(),
                });

                // Create a client using the leased key
                let client_result = client::create_client(&ProviderConfig {
                    provider,
                    adapter,
                    api_key: lease.key.clone(),
                });
                let client = match client_result {
                    Ok(c) => c,
                    Err(e) => {
                        let err = e.to_string();
                        let _ = tx.send(TuiEvent::CaseFinished {
                            case_id: eval_case.id.clone(),
                            pass: false,
                            duration_ms: 0,
                            detail: crate::evals_tui::CaseDetail {
                                error: Some(err.clone()),
                                ..Default::default()
                            },
                        });
                        return (eval_case, model, 0u64, Err(err));
                    }
                };

                // Set up tool progress channel
                let (tool_tx, mut tool_rx) = mpsc::unbounded_channel::<(String, bool)>();
                let case_id_for_progress = eval_case.id.clone();
                let tx_for_progress = tx.clone();

                // Spawn a task to forward tool progress events to the TUI
                let progress_forwarder = tokio::spawn(async move {
                    while let Some((tool_name, ok)) = tool_rx.recv().await {
                        let _ = tx_for_progress.send(TuiEvent::ToolDone {
                            case_id: case_id_for_progress.clone(),
                            tool_name,
                            ok,
                        });
                    }
                });

                let started = Instant::now();
                let result = run_case_with_progress(
                    &client,
                    &model,
                    &dispatcher,
                    &eval_case,
                    langsmith.as_ref(),
                    &tool_tx,
                )
                .await;
                let elapsed_ms = started.elapsed().as_millis() as u64;

                // Close the tool progress channel and wait for forwarder to drain
                drop(tool_tx);
                let _ = progress_forwarder.await;

                // Send case finished event with detail
                let (pass, detail) = match &result {
                    Ok(run) => {
                        let grade = grade_case(&eval_case, run);
                        let detail = crate::evals_tui::CaseDetail {
                            error: None,
                            response: Some(run.response.clone()),
                            tier_a: Some(grade.detail_a),
                            tier_b: Some(grade.detail_b),
                            tier_c: Some(grade.detail_c),
                        };
                        (grade.pass, detail)
                    }
                    Err(e) => {
                        let detail = crate::evals_tui::CaseDetail {
                            error: Some(e.clone()),
                            ..Default::default()
                        };
                        (false, detail)
                    }
                };
                let _ = tx.send(TuiEvent::CaseFinished {
                    case_id: eval_case.id.clone(),
                    pass,
                    duration_ms: elapsed_ms,
                    detail,
                });

                // Drop the lease to return the key to the pool
                drop(lease);

                (eval_case, model, elapsed_ms, result)
            });
        }

        // Collect one completed result, or bail on cancel
        if !join_set.is_empty() {
            tokio::select! {
                Some(joined) = join_set.join_next() => {
                    match joined {
                        Ok((eval_case, model, elapsed_ms, result)) => {
                            let eval_result = build_eval_result_quiet(eval_case, model, elapsed_ms, result);
                            write_result_file(&run_dir, &eval_result);
                            results.push(eval_result);
                        }
                        Err(_) if cancel.is_cancelled() => break,
                        Err(e) => return Err(format!("parallel task join error: {e}")),
                    }
                }
                _ = cancel.cancelled() => {
                    join_set.abort_all();
                    break;
                }
            }
        }
    }

    let _ = tui_tx.send(TuiEvent::AllDone);
    Ok(results)
}

/// Like [`run_case`] but passes the tool progress sender through to the agent.
async fn run_case_with_progress(
    llm_client: &LlmClient,
    model: &str,
    dispatcher: &ToolDispatcher,
    eval_case: &EvalCase,
    langsmith: Option<&LangSmithConfig>,
    tool_tx: &mpsc::UnboundedSender<(String, bool)>,
) -> Result<AgentCaseRun, String> {
    let messages = vec![Message {
        role: "user".to_string(),
        content: crate::agent::types::Content::Text(eval_case.query.clone()),
    }];
    let started = Instant::now();
    let (agent_tx, mut agent_rx) = mpsc::unbounded_channel::<crate::agent::AgentEvent>();
    let tool_tx_forward = tool_tx.clone();
    let tool_forwarder = tokio::spawn(async move {
        while let Some(event) = agent_rx.recv().await {
            if let crate::agent::AgentEvent::ToolCall(tc) = event {
                let _ = tool_tx_forward.send((tc.name, tc.success));
            }
        }
    });

    let result = agent::run_with_dispatcher(
        llm_client,
        model,
        messages,
        dispatcher,
        langsmith,
        Some(&agent_tx),
    )
    .await
    .map_err(|e| e.to_string())?;
    drop(agent_tx);
    let _ = tool_forwarder.await;

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
        verification: result.verification,
        duration_ms: started.elapsed().as_millis() as u64,
        input_tokens: result.input_tokens,
        output_tokens: result.output_tokens,
    })
}

/// Like [`build_eval_result`] but without printing (used in TUI mode).
fn build_eval_result_quiet(
    eval_case: EvalCase,
    model: String,
    elapsed_ms: u64,
    result: Result<AgentCaseRun, String>,
) -> EvalResult {
    match result {
        Ok(run) => {
            let grade = grade_case(&eval_case, &run);
            EvalResult {
                case_id: eval_case.id,
                model,
                description: eval_case.description,
                query: eval_case.query,
                category: eval_case.category,
                difficulty: eval_case.difficulty,
                tags: eval_case.tags,
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
                verification: Some(run.verification.clone()),
                duration_ms: run.duration_ms,
                input_tokens: run.input_tokens,
                output_tokens: run.output_tokens,
                timestamp: Utc::now().to_rfc3339(),
                error: None,
                steps: run.steps,
            }
        }
        Err(e) => EvalResult {
            case_id: eval_case.id,
            model,
            description: eval_case.description,
            query: eval_case.query,
            category: eval_case.category,
            difficulty: eval_case.difficulty,
            tags: eval_case.tags,
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
            verification: None,
            duration_ms: elapsed_ms,
            input_tokens: 0,
            output_tokens: 0,
            timestamp: Utc::now().to_rfc3339(),
            error: Some(e),
            steps: Vec::new(),
        },
    }
}

async fn run_case(
    llm_client: &LlmClient,
    model: &str,
    dispatcher: &ToolDispatcher,
    eval_case: &EvalCase,
    langsmith: Option<&LangSmithConfig>,
) -> Result<AgentCaseRun, String> {
    let messages = vec![Message {
        role: "user".to_string(),
        content: crate::agent::types::Content::Text(eval_case.query.clone()),
    }];
    let started = Instant::now();
    let result =
        agent::run_with_dispatcher(llm_client, model, messages, dispatcher, langsmith, None)
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
        verification: result.verification,
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

struct GradeInput<'a> {
    tools_called: &'a [String],
    response: &'a str,
    verified: bool,
}

fn grade_case(eval_case: &EvalCase, run: &AgentCaseRun) -> Grade {
    let input = GradeInput {
        tools_called: &run.tools_called,
        response: &run.response,
        verified: run.verified,
    };
    grade_case_with_input(eval_case, &input)
}

fn grade_case_with_input(eval_case: &EvalCase, input: &GradeInput<'_>) -> Grade {
    let (tier_a, detail_a) = grade_tier_a(eval_case, input);
    let (tier_b, detail_b) = grade_tier_b(eval_case, input);
    let (tier_c, detail_c) = grade_tier_c(eval_case, input);
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

fn grade_tier_a(eval_case: &EvalCase, input: &GradeInput<'_>) -> (bool, String) {
    let must_contain = if eval_case.tool_must_contain.is_empty() {
        &eval_case.expected_tools
    } else {
        &eval_case.tool_must_contain
    };

    let required: HashSet<String> = must_contain.iter().map(|t| t.to_string()).collect();
    let forbidden: HashSet<String> = eval_case
        .tool_must_not_contain
        .iter()
        .map(|t| t.to_string())
        .collect();
    let actual: HashSet<String> = input.tools_called.iter().map(|t| t.to_string()).collect();

    if required.is_empty() && forbidden.is_empty() && actual.is_empty() {
        return (true, "No tools expected, none called".to_string());
    }

    let missing: Vec<String> = required
        .iter()
        .filter(|tool| !actual.contains(*tool))
        .cloned()
        .collect();

    let forbidden_called: Vec<String> = actual
        .iter()
        .filter(|tool| forbidden.contains(*tool))
        .cloned()
        .collect();

    if missing.is_empty() && forbidden_called.is_empty() {
        return (true, "All expected tools were called".to_string());
    }

    let mut details = Vec::new();
    if !missing.is_empty() {
        details.push(format!("Missing tools [{}]", missing.join(", ")));
    }
    if !forbidden_called.is_empty() {
        details.push(format!(
            "Forbidden tools called [{}]",
            forbidden_called.join(", ")
        ));
    }

    (
        false,
        format!(
            "{}; called [{}]",
            details.join("; "),
            actual.into_iter().collect::<Vec<_>>().join(", ")
        ),
    )
}

fn grade_tier_b(eval_case: &EvalCase, input: &GradeInput<'_>) -> (bool, String) {
    let lower = input.response.to_lowercase();
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
    if !eval_case.must_contain_any.is_empty()
        && !eval_case
            .must_contain_any
            .iter()
            .any(|s| lower.contains(&s.to_lowercase()))
    {
        return (
            false,
            format!(
                "Missing any-of in response [{}]",
                eval_case.must_contain_any.join(", ")
            ),
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

fn grade_tier_c(eval_case: &EvalCase, input: &GradeInput<'_>) -> (bool, String) {
    if eval_case.skip_verified {
        return (true, format!("verified={} (skipped)", input.verified));
    }
    if input.verified == eval_case.expected_verified {
        return (true, format!("verified={} as expected", input.verified));
    }
    (
        false,
        format!(
            "Expected verified={}, got {}",
            eval_case.expected_verified, input.verified
        ),
    )
}

pub fn replay(args: ReplayArgs) -> Result<(), String> {
    let evals_root = resolve_evals_root(args.evals_root)?;
    let source_run_dir = if let Some(path) = args.path {
        if !path.is_dir() {
            return Err(format!("replay path is not a directory: {}", path.display()));
        }
        path
    } else {
        resolve_run_dir(&evals_root, args.run_id.as_deref())?
    };

    let source_run_id = source_run_dir
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("unknown-run")
        .to_string();

    let source_cases = load_replay_source_cases(&source_run_dir)?;
    if source_cases.is_empty() {
        return Err(format!(
            "no replay case files found in {}",
            source_run_dir.display()
        ));
    }

    let all_cases = load_cases(&evals_root)?;
    let case_by_id: std::collections::HashMap<String, EvalCase> = all_cases
        .into_iter()
        .map(|case| (case.id.clone(), case))
        .collect();

    let run_id = format!(
        "replay-{}-{}",
        source_run_id,
        Utc::now().format("%Y%m%d-%H%M%S")
    );
    let run_dir = evals_root.join("results").join(&run_id);
    create_dir_all(&run_dir).map_err(|e| e.to_string())?;

    println!(
        "Replay source: {} ({} cases)",
        source_run_dir.display(),
        source_cases.len()
    );
    println!("Replay run: {}", run_dir.display());

    let started = Instant::now();
    let mut results = Vec::with_capacity(source_cases.len());
    let mut improved = 0usize;
    let mut regressed = 0usize;
    let mut unchanged_pass = 0usize;
    let mut unchanged_fail = 0usize;

    for source in source_cases {
        let eval_result = if let Some(eval_case) = case_by_id.get(&source.case_id) {
            let input = GradeInput {
                tools_called: &source.tools_called,
                response: &source.response,
                verified: source.verified,
            };
            let grade = grade_case_with_input(eval_case, &input);

            if source.pass && !grade.pass {
                regressed += 1;
            } else if !source.pass && grade.pass {
                improved += 1;
            } else if source.pass {
                unchanged_pass += 1;
            } else {
                unchanged_fail += 1;
            }

            EvalResult {
                case_id: eval_case.id.clone(),
                model: source.model.clone(),
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
                tools_called: source.tools_called.clone(),
                response: source.response.clone(),
                verified: source.verified,
                verification: None,
                duration_ms: source.duration_ms,
                input_tokens: source.input_tokens,
                output_tokens: source.output_tokens,
                timestamp: Utc::now().to_rfc3339(),
                error: source.error.clone(),
                steps: Vec::new(),
            }
        } else {
            regressed += 1;
            EvalResult {
                case_id: source.case_id.clone(),
                model: source.model.clone(),
                description: "(missing case definition)".to_string(),
                query: String::new(),
                category: "unknown".to_string(),
                difficulty: "unknown".to_string(),
                tags: Vec::new(),
                pass: false,
                tier_a: false,
                tier_b: false,
                tier_c: false,
                detail_a: "N/A".to_string(),
                detail_b: "N/A".to_string(),
                detail_c: "N/A".to_string(),
                tools_called: source.tools_called.clone(),
                response: source.response.clone(),
                verified: source.verified,
                verification: None,
                duration_ms: source.duration_ms,
                input_tokens: source.input_tokens,
                output_tokens: source.output_tokens,
                timestamp: Utc::now().to_rfc3339(),
                error: Some(format!(
                    "case id '{}' no longer exists in eval definitions",
                    source.case_id
                )),
                steps: Vec::new(),
            }
        };

        write_result_file(&run_dir, &eval_result);
        results.push(eval_result);
    }

    results.sort_by(|a, b| {
        a.model
            .cmp(&b.model)
            .then_with(|| a.case_id.cmp(&b.case_id))
    });

    let total = results.len();
    let passed = results.iter().filter(|r| r.pass).count();
    let failed = total - passed;
    let errors = results.iter().filter(|r| r.error.is_some()).count();
    let total_input_tokens: u64 = results.iter().map(|r| r.input_tokens).sum();
    let total_output_tokens: u64 = results.iter().map(|r| r.output_tokens).sum();
    let replay_duration_ms = started.elapsed().as_millis() as u64;
    let (latency_p50_ms, latency_p95_ms) =
        compute_latency_percentiles(results.iter().map(|r| r.duration_ms));
    println!(
        "Replay summary: total={} passed={} failed={} errors={} input_tokens={} output_tokens={} duration={} p50={} p95={}",
        total,
        passed,
        failed,
        errors,
        total_input_tokens,
        total_output_tokens,
        format_seconds(replay_duration_ms),
        format_seconds(latency_p50_ms),
        format_seconds(latency_p95_ms)
    );
    println!(
        "Replay diff: unchanged_pass={} unchanged_fail={} improved={} regressed={}",
        unchanged_pass, unchanged_fail, improved, regressed
    );

    print_model_summary(&results);
    write_results_jsonl(&evals_root, &run_id, &results)?;
    write_summary_file(
        &evals_root,
        &run_id,
        &format!("replay:{source_run_id}"),
        total,
        passed,
        failed,
        errors,
        total_input_tokens,
        total_output_tokens,
        replay_duration_ms,
        latency_p50_ms,
        latency_p95_ms,
    )?;
    let models: std::collections::BTreeSet<String> =
        results.iter().map(|r| r.model.clone()).collect();
    write_results_sqlite(
        &evals_root,
        &run_id,
        &format!("replay:{source_run_id}"),
        &models.into_iter().collect::<Vec<_>>().join(", "),
        replay_duration_ms,
        latency_p50_ms,
        latency_p95_ms,
        &results,
    )?;

    Ok(())
}

fn write_result_file(run_dir: &Path, result: &EvalResult) {
    let path = run_dir.join(format!("{}.json", result.case_id));
    match serde_json::to_string_pretty(result) {
        Ok(body) => {
            if let Err(e) = std::fs::write(&path, body) {
                eprintln!("warn: failed to write {}: {e}", path.display());
            }
        }
        Err(e) => eprintln!("warn: failed to serialize {}: {e}", result.case_id),
    }
}

fn write_results_jsonl(
    evals_root: &Path,
    run_id: &str,
    results: &[EvalResult],
) -> Result<(), String> {
    // Individual case files are already written incrementally by write_result_file.
    // This function ensures the dir exists and reports the final count.
    let run_dir = evals_root.join("results").join(run_id);
    create_dir_all(&run_dir).map_err(|e| e.to_string())?;
    println!("Results: {} cases in {}/", results.len(), run_dir.display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_summary_file(
    evals_root: &Path,
    run_id: &str,
    suite: &str,
    total: usize,
    passed: usize,
    failed: usize,
    errors: usize,
    input_tokens: u64,
    output_tokens: u64,
    duration_ms: u64,
    latency_p50_ms: u64,
    latency_p95_ms: u64,
) -> Result<(), String> {
    let run_dir = evals_root.join("results").join(run_id);
    create_dir_all(&run_dir).map_err(|e| e.to_string())?;
    let summary = RunSummaryFile {
        run_id: run_id.to_string(),
        suite: suite.to_string(),
        total,
        passed,
        failed,
        errors,
        input_tokens,
        output_tokens,
        duration_ms,
        latency_p50_ms,
        latency_p95_ms,
    };
    let path = run_dir.join("summary.json");
    let body =
        serde_json::to_string_pretty(&summary).map_err(|e| format!("serialize summary: {e}"))?;
    std::fs::write(&path, body).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

fn write_results_sqlite(
    evals_root: &Path,
    run_id: &str,
    suite: &str,
    model: &str,
    duration_ms: u64,
    latency_p50_ms: u64,
    latency_p95_ms: u64,
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
  latency_p50_ms INTEGER NOT NULL DEFAULT 0,
  latency_p95_ms INTEGER NOT NULL DEFAULT 0,
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
    ensure_runs_column(&conn, "latency_p50_ms", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_runs_column(&conn, "latency_p95_ms", "INTEGER NOT NULL DEFAULT 0")?;

    let total = results.len() as i64;
    let passed = results.iter().filter(|r| r.pass).count() as i64;
    let failed = total - passed;
    let errors = results.iter().filter(|r| r.error.is_some()).count() as i64;
    let tokens_in: i64 = results.iter().map(|r| r.input_tokens as i64).sum();
    let tokens_out: i64 = results.iter().map(|r| r.output_tokens as i64).sum();
    let timestamp = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO runs (run_id, suite, model_id, model_label, total, passed, failed, errors, duration_ms, latency_p50_ms, latency_p95_ms, tokens_in, tokens_out, timestamp)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
            latency_p50_ms as i64,
            latency_p95_ms as i64,
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

fn ensure_runs_column(conn: &Connection, column: &str, spec: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(runs)")
        .map_err(|e| format!("failed to inspect runs schema: {e}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| format!("failed to query runs schema: {e}"))?;
    let mut present = false;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let name: String = row.get(1).map_err(|e| e.to_string())?;
        if name == column {
            present = true;
            break;
        }
    }
    if !present {
        conn.execute(
            &format!("ALTER TABLE runs ADD COLUMN {column} {spec}"),
            [],
        )
        .map_err(|e| format!("failed to migrate runs.{column}: {e}"))?;
    }
    Ok(())
}

/// Resolve the adapter for a given provider (mirrors Config logic).
fn provider_adapter(provider: Provider, cfg: &Config) -> Adapter {
    match provider {
        Provider::Anthropic => Adapter::AnthropicMessages,
        Provider::OpenRouter | Provider::OpenAI => {
            // Respect the configured adapter, default to ChatCompletions
            let value = std::env::var(match provider {
                Provider::OpenRouter => "OPENROUTER_ADAPTER",
                _ => "OPENAI_ADAPTER",
            })
            .ok()
            .or_else(|| cfg.llm_adapter.clone());
            value
                .as_deref()
                .and_then(Adapter::parse)
                .unwrap_or(Adapter::OpenAIChatCompletions)
        }
    }
}

fn compute_latency_percentiles<I>(durations_ms: I) -> (u64, u64)
where
    I: IntoIterator<Item = u64>,
{
    let mut samples: Vec<u64> = durations_ms.into_iter().filter(|ms| *ms > 0).collect();
    if samples.is_empty() {
        return (0, 0);
    }
    samples.sort_unstable();
    let p50 = samples[percentile_index(samples.len(), 50)];
    let p95 = samples[percentile_index(samples.len(), 95)];
    (p50, p95)
}

fn percentile_index(len: usize, percentile: usize) -> usize {
    let rank = (len * percentile).div_ceil(100);
    rank.saturating_sub(1).min(len.saturating_sub(1))
}

#[cfg(test)]
#[path = "evals_test.rs"]
mod tests;
