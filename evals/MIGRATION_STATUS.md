# Evals Migration Status

The eval system migration is complete and CLI-first.

## Current State

- Eval definitions live in `cli/evals/` (single source of truth).
- Primary command is `ghostfolio evals`.
- Legacy TypeScript runner and `gauntlet/evals/` have been removed.

## Entrypoints

- From `gauntlet/`:
  - `npm run eval`
  - Runs: `cargo run --manifest-path ../cli/Cargo.toml -- evals --suite comprehensive`
- Direct:
  - `cargo run --manifest-path cli/Cargo.toml -- evals --suite comprehensive`

## Remaining Functional Gaps

- Tier D rubric scorer (LLM-as-judge) not yet implemented.
- `--replay` flag not yet implemented.
- `empty-portfolio` and `error-states` edge-case fixture sets not yet created.
- `must_contain`/`must_not_contain` values not yet updated to exact fixture values in all cases.
