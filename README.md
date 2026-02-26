# Ghostfolio CLI (Rust)

This crate contains the Rust TUI/agent CLI implementation.

## Commands

Build and run the app:

```bash
cd cli
cargo run
```

## Testing

### Rust unit tests (code tests)

These run Rust unit tests (`*_test.rs`) for CLI code.

```bash
cd cli
cargo test
```

Run a subset:

```bash
cargo test tools::calculator
```

Coverage:

```bash
cargo llvm-cov --summary-only
```

### Evals (agent behavior tests)

The CLI subcommand named `test` runs eval suites, not Rust unit tests.

```bash
cd cli
cargo run -- test --suite quick
```

Direct binary form:

```bash
ghostfolio test --suite quick
```

For eval fixture/suite details, see [evals/README.md](./evals/README.md).

## Naming note

`test` is currently the eval subcommand name. Renaming it to `evals` would reduce confusion with Rust `cargo test`.
