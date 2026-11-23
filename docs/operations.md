# Operations

## Local development

Prerequisites: Rust (see `rust-toolchain.toml`), `task` (Taskfile runner), and for
full probe functionality the external scanners: subfinder, dnsx, httpx, an RDAP
client, dig, asnmap. Tests do not require network access or these tools.

Canonical commands (see `Taskfile.yml`):

```text
task fmt        # cargo fmt --all
task fmt:check  # formatting gate
task lint       # clippy --all-targets --locked -- -D warnings
task test       # cargo test --locked
task doc        # rustdoc with warnings as errors
task check      # fmt:check + lint + doc + test (the full local gate)
task build      # cargo build
task smoke      # fixture-based smoke test (no network)
```

Every commit must pass `task check`. CI runs the same gate
(`.github/workflows/ci.yml`); local and CI toolchains match via
`rust-toolchain.toml` and `Cargo.lock`.

## Quality gate

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
cargo test --locked
```

Strict Clippy configuration lives in `clippy.toml` (no unwrap/expect/panic outside
tests). Formatting configuration lives in `rustfmt.toml`.

## Release procedure

Recorded in `docs/adr/` decisions and executed via the `balebos-release-check`
skill (`.agents/skills/balebos-release-check/`). In short:

1. Confirm product-scope compliance and migration instructions (`docs/migration-v2.md`).
2. Run the full quality gate and fixture-based smoke tests.
3. Verify CLI behaviour, TUI behaviour, exports and exit codes.
4. Update CHANGELOG and version (SemVer; Conventional Commits drive the log).
5. Tag only from `main` after merging a `release/*` branch; never tag an
   incomplete migration as stable.

## Storage data

Operator data lives under `data/` (gitignored). Current-state layout:

- `data/balebos.db` — SQLite operational store (target default; opt in
  with `BALEBOS_STORAGE=sqlite`).
- `data/parquet/` — legacy 1.x Parquet store (default until migration is
  exercised; `balebos migrate` imports it and renames the directory).
- `data/archive/` — sealed evidence segments and manifests (written by
  `balebos archive build` in `parquet-archive` builds).

Legacy formats, migration procedure and rollback options are described in
`docs/migration-v2.md`. Never delete historical observations during
upgrades.
