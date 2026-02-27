# Ghostfolio Evals (CLI)

This directory contains the eval corpus used by the Rust CLI eval runner (`ghostfolio evals`).

## Layout

```
evals/
  golden_sets/        Stage 1 regression tests (single-tool happy paths)
  scenarios/          Stage 2 coverage tests (multi-tool, adversarial, portfolio)
  rubrics/            Rubric config (Tier D planned)
  fixtures/           Fixture-backed API responses for mock mode
  suites.yaml         Suite definitions (quick, comprehensive, etc.)
  models.yaml         Model registry used for eval matrix runs
  results/            NDJSON + SQLite artifacts
```

## Running Evals

From repo root:

```bash
cargo run --manifest-path cli/Cargo.toml -- evals --suite comprehensive
```

From `gauntlet/`:

```bash
npm run eval
```

List suites:

```bash
cargo run --manifest-path cli/Cargo.toml -- evals --list-suites
```

Quick multi-model run:

```bash
cargo run --manifest-path cli/Cargo.toml -- evals --suite quick --models openai/gpt-4o-mini,claude-sonnet-4-6 --parallel --max-parallel 4
```

Live API mode:

```bash
cargo run --manifest-path cli/Cargo.toml -- evals --suite quick --live --model openai/gpt-4o-mini
```

## Flags

| Flag             | Argument    | Default                | Description                                                  |
| ---------------- | ----------- | ---------------------- | ------------------------------------------------------------ |
| `--suite`        | name        | `quick`                | Suite ID from `suites.yaml`                                  |
| `--models`       | id1,id2     | configured model       | Comma-separated model IDs                                    |
| `--model`        | id          | configured model       | Single model ID (conflicts with `--models`)                  |
| `--provider`     | provider id | auto                   | Provider override (`anthropic`, `openrouter`, `openai`)      |
| `--case`         | id1,id2     | suite case selection   | Override suite case selection                                |
| `--parallel`     | —           | `false`                | Run cases in parallel                                        |
| `--max-parallel` | number      | CPU parallelism        | Max concurrency when `--parallel` is enabled                 |
| `--list-suites`  | —           | —                      | Print suites and exit                                        |
| `--live`         | —           | `false` (fixtures)     | Use live Ghostfolio API calls instead of fixture-backed mode |

## Outputs

- Per-case JSON files: `evals/results/<run-id>/<case-id>.json`
- SQLite DB: `evals/results/results.db`

## Notes

- `ghostfolio evals` is distinct from Rust unit tests (`cargo test`).
- Migration status and known gaps: `MIGRATION_STATUS.md`.
