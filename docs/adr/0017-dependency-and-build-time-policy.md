# 0017 — Dependency and build-time policy

**Status:** Accepted

## Decision

Compile cost is a budgeted resource; dependencies pay rent.

- Every dependency must justify its compile time, binary size, licence,
  maintenance status and security surface against a concrete application
  need. "Already present" is not a justification; "popular" is not a need.
- **Default features are opt-in, not opt-out by accident:** heavy crates
  are declared with `default-features = false` and only required features
  enabled (the Polars default-features leak, Phase A, is the canonical
  counter-example).
- Analytics/DataFrame engines (Polars, DataFusion, DuckDB) are prohibited
  from the default build. Columnar work uses the `parquet` crate directly,
  behind the optional `parquet-archive` feature (ADR 0013).
- Cloud SDKs are prohibited from the default build (ADR 0016).
- Measurements accompany removals: cold/incremental build time, binary
  size and dependency count are recorded before and after (Phase A
  baseline vs Phase E result) — estimates are not claims.

## Context

Phase A measured 424 packages and a Polars-pulled tokio/hyper/reqwest chain
serving no application need. The 2.0 brief makes build weight an explicit
quality attribute of a small local-first tool.

## Consequences

- `cargo build --timings` and package counts are part of the review
  evidence for any dependency change.
- CI keeps `--locked` so the budget is reproducible.
- New heavy dependencies require an ADR referencing this policy.
