---
name: balebos-release-check
description: >-
  Pre-release verification for Balebos: quality gate, migration instructions,
  CLI/TUI behaviour, export contracts, documentation and packaging. Use before
  tagging any release or pre-release.
---

# Balebos Release Check

Work through the list in order; stop and report at the first blocking failure.
Produce a written checklist result suitable for pasting into a release PR.

## 1. Product-scope compliance

- No Angry Turnip functionality (SCA/CVE/secrets/SBOM/licences/CI auditing).
- No enterprise governance or distributed-infrastructure creep.
- New features map to the five fundamental questions in `docs/product.md`.

## 2. Version compatibility

- Version in `Cargo.toml` matches CHANGELOG; SemVer respected; Conventional
  Commits drive the log.
- Migration path from the previous release documented in `docs/migration-v2.md`;
  data migration tooling tested; rollback options stated.

## 3. CLI behaviour

- `task check` green (fmt, clippy `-D warnings`, rustdoc `-D warnings`, tests).
- Help text accurate; error messages en-GB, no internal details leaked.
- stdout carries results only; diagnostics on stderr.
- Exit codes match `src/exit_codes.rs` semantics: execution failure ≠
  operational finding; partial scans identifiable.

## 4. TUI behaviour

- Launches, navigates, restores the terminal on exit and on panic paths.
- No business-rule divergence from CLI (shared services).
- Active filters and scan coverage visible; narrow-terminal layout sane.

## 5. Exports and integration contracts

- Versioned export schemas documented (ADR 0010) and stable within the version.
- Fixture round-trip: export → independent consume (script/CI example) succeeds
  without Balebos internals.
- No silent record dropping in export paths.

## 6. Storage and migration

- Failure-injection tests pass (interrupted write, corrupt record, restart).
- No silent data loss paths (per `balebos-storage-review`).
- Legacy 1.x data imports losslessly with verification.

## 7. Documentation

- `docs/` current; ADRs updated or added for new decisions; superseded docs
  marked, not rewritten silently.
- Known limitations listed in the release notes. No documented feature that
  does not exist.

## 8. Packaging and build

- Clean-checkout build works: `cargo build --release --locked`.
- Toolchain pinned (`rust-toolchain.toml`); Cargo.lock committed and current.
- Docker build works if the image is part of the release.

## Output

```text
Release check <version>
[PASS/FAIL] 1. Product scope
...
Blocking failures: n
Verdict: RELEASABLE / NOT RELEASABLE
```
