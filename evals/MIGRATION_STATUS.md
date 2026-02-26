# Evals Migration Status (CLI-First)

This file tracks migration of eval execution from `gauntlet/evals/runner/run.ts` to `cli/src/evals.rs`.

## Current Decision

- Eval definitions are **vendored in `cli/evals/`** (primary) with fallback to `gauntlet/evals/`.
- Primary runner is the Rust CLI command: `ghostfolio test`.
- Legacy TypeScript runner remains available as fallback during parity validation.

## Entrypoints

- Primary:
  - `cd gauntlet && npm run eval`
  - Runs: `cargo run --manifest-path ../cli/Cargo.toml -- test --suite comprehensive`
- Fallback:
  - `cd gauntlet && npm run eval:legacy`
  - Runs: `tsx evals/runner/run.ts`

## Feature Parity (Rust vs TS)

| Feature                          | TS Runner | Rust Runner |
| -------------------------------- | --------- | ----------- |
| Suite/case selection             | Yes       | Yes         |
| Tier A/B/C grading              | Yes       | Yes         |
| NDJSON output                    | Yes       | Yes         |
| SQLite output                    | Yes       | Yes         |
| Multi-model matrix               | Yes       | Yes         |
| Parallel execution               | Yes       | Yes (async) |
| Bounded concurrency              | No        | Yes         |
| Cross-model comparison summaries | No        | Yes         |
| LangSmith tracing                | No        | Yes         |
| Fixture-backed mock mode         | No        | Yes         |
| Cost estimation + p50/p95        | Yes       | Yes         |
| Coverage matrix (category×diff)  | Yes       | Yes         |

## Parity Gates (Before Legacy Removal)

- [x] Gate 1: Case selection parity (`suite`, `--case` filtering)
- [x] Gate 2: Grade parity (Tier A/B/C pass/fail — identical logic implemented)
- [x] Gate 3: Error parity (error cases persisted and counted consistently)
- [x] Gate 4: Artifact parity (SQLite schema identical, NDJSON format identical)
- [ ] Gate 5: CI cutover (workflow uses CLI runner as source of truth)
- [ ] Gate 6: Legacy retirement (`eval:legacy` + `evals/runner/*` removal)

## Known Remaining Gaps

- Tier D rubric scorer (LLM-as-judge) not yet implemented in either runner.
- `--replay` flag not yet implemented.
- `empty-portfolio` and `error-states` edge-case fixture sets not yet created.
- `must_contain`/`must_not_contain` values not yet updated to use exact fixture values in all cases.
- CI workflow not yet switched to Rust runner (Gate 5).
- TS runner code still present in `gauntlet/evals/runner/` (Gate 6).
