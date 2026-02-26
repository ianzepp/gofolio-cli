# CLI Repository Strategy

## Current Setup

The Rust CLI source lives at `cli/` inside the `ianzepp-gauntlet/ghostfolio` monorepo. This is a grading requirement — the evaluation tool clones a single repo and cannot follow submodules or multi-repo setups.

The CLI is structurally and logically independent from the Ghostfolio TypeScript project:

- Different language (Rust vs TypeScript/Angular)
- No source dependencies on anything outside `cli/`
- Own `Cargo.toml`, own test suite, own eval harness (`cli/evals/`)
- Own release workflow (cross-platform binaries + Homebrew)

## Release Repo

Releases are published to **`ianzepp/ghostfolio-cli`** via the GitHub Action at `.github/workflows/release-cli.yml`:

1. Tag `cli-v*` on this repo triggers the workflow
2. Builds binaries for Linux (x86_64), macOS (ARM + x86_64), Windows (x86_64)
3. Creates a GitHub Release on `ianzepp/ghostfolio-cli` with the binaries
4. Updates the Homebrew formula in `ianzepp/homebrew-tap`

The release repo currently only holds release assets, not source code.

## Future: Source Mirroring

To make `ianzepp/ghostfolio-cli` a full standalone repo (source + releases), mirror the `cli/` subtree on each push:

```bash
# Manual one-off
git subtree push --prefix=cli git@github.com:ianzepp/ghostfolio-cli.git main
```

Or automate with a GitHub Action that runs on push to `main`:

```yaml
- name: Mirror cli/ to ghostfolio-cli repo
  run: |
    git subtree split --prefix=cli -b cli-mirror
    git push https://x-access-token:${{ secrets.GHOSTFOLIO_CLI_RELEASE_TOKEN }}@github.com/ianzepp/ghostfolio-cli.git cli-mirror:main --force
    git branch -D cli-mirror
```

### Why Not Submodules

- Grading tool may not clone with `--recursive`
- Adds friction for contributors (submodule init/update dance)
- Source of truth stays in the monorepo where graders expect it

### Why Mirror Instead

- `cli/` stays in this repo as a normal directory — nothing changes for graders
- `ianzepp/ghostfolio-cli` becomes a standalone repo for independent use, stars, and discoverability
- Release workflow already targets that repo, so source + binaries live together
- No risk of breaking the grading flow
