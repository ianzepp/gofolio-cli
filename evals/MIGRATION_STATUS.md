# Evals Migration Status (CLI-First)

This file tracks migration of eval execution from `gauntlet/evals/runner/run.ts` to `cli/src/evals.rs`.

## Current Decision

- YAML test definitions in `gauntlet/evals/` are now treated as **CLI-owned execution input**.
- Primary runner is the Rust CLI command: `ghostfolio test`.
- Legacy TypeScript runner remains available as fallback during parity validation.

## Entrypoints

- Primary:
  - `cd gauntlet && npm run eval`
  - Runs: `cargo run --manifest-path ../cli/Cargo.toml -- test --suite comprehensive`
- Fallback:
  - `cd gauntlet && npm run eval:legacy`
  - Runs: `tsx evals/runner/run.ts`

## Parity Gates (Before Legacy Removal)

- [ ] Gate 1: Case selection parity (`suite`, `--case` filtering) on a fixed suite subset
- [ ] Gate 2: Grade parity (Tier A/B/C pass/fail deltas reviewed and accepted)
- [ ] Gate 3: Error parity (error cases persisted and counted consistently)
- [ ] Gate 4: Artifact parity (SQLite/NDJSON fields consumed by scripts/docs)
- [ ] Gate 5: CI cutover (workflow uses CLI runner as source of truth)
- [ ] Gate 6: Legacy retirement (`eval:legacy` + `evals/runner/*` removal)

## Known Remaining Gaps

- LangSmith tracing is not yet enabled for `ghostfolio test`.
- Some docs/scripts still describe TS-runner-only flags or behavior.
- TS-only model registry flow (`models.yaml` labels) is not yet mirrored by CLI.
