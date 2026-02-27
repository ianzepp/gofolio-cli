use std::path::PathBuf;

use super::{TestArgs, run};

fn deterministic_args(suite: &str, case_ids: Option<Vec<&str>>) -> TestArgs {
    TestArgs {
        suite: suite.to_string(),
        case_ids: case_ids.map(|ids| ids.into_iter().map(str::to_string).collect()),
        model: Some("openai/gpt-4o-mini".to_string()),
        models: None,
        provider: Some("openrouter".to_string()),
        evals_root: Some(PathBuf::from("evals")),
        fixture_dir: None,
        live: false,
        parallel: false,
        max_parallel: Some(1),
        list_suites: false,
        no_tui: true,
    }
}

#[tokio::test]
#[ignore = "manual deterministic eval lane"]
async fn golden_smoke_subset() {
    let args = deterministic_args("quick", Some(vec!["acct-001", "mkt-001", "fx-001"]));
    run(args).await.expect("deterministic golden smoke failed");
}

async fn run_single_quick_case(case_id: &str) {
    let args = deterministic_args("quick", Some(vec![case_id]));
    run(args)
        .await
        .unwrap_or_else(|e| panic!("deterministic quick case '{case_id}' failed: {e}"));
}

macro_rules! ignored_quick_case {
    ($name:ident, $case_id:literal) => {
        #[tokio::test]
        #[ignore = "manual deterministic eval lane"]
        async fn $name() {
            run_single_quick_case($case_id).await;
        }
    };
}

ignored_quick_case!(quick_case_acct_001, "acct-001");
ignored_quick_case!(quick_case_mkt_001, "mkt-001");
ignored_quick_case!(quick_case_fx_001, "fx-001");
ignored_quick_case!(quick_case_sym_001, "sym-001");
ignored_quick_case!(quick_case_prof_001, "prof-001");
ignored_quick_case!(quick_case_bench_001, "bench-001");
ignored_quick_case!(quick_case_hist_001, "hist-001");
ignored_quick_case!(quick_case_hold_001, "hold-001");
ignored_quick_case!(quick_case_hold_002, "hold-002");
ignored_quick_case!(quick_case_div_001, "div-001");
ignored_quick_case!(quick_case_inv_001, "inv-001");
ignored_quick_case!(quick_case_act_001, "act-001");
ignored_quick_case!(quick_case_bal_001, "bal-001");
ignored_quick_case!(quick_case_spark_001, "spark-001");
ignored_quick_case!(quick_case_bar_001, "bar-001");
