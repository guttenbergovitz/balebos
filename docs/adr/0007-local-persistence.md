# 0007 — Local persistence and migration

**Status:** Proposed

## Context

The 1.x Parquet engine (audit §5, verified) stores JSON strings in a single
`raw_json` column, appends by reading and rewriting entire files, lacks atomic
commit, coerces unknown record types to `Domain`, silently skips malformed
records, and loads all history into memory per query. The Polars/Arrow stack is
the heaviest dependency in the tree yet provides only compression. The brief
requires: crash-consistent persistence, explicit flush/commit, reliable recovery,
bounded memory, versioned schemas, migration support, deterministic export,
explicit corruption detection, no silent data loss.

## Alternatives

- **A. SQLite** (via `rusqlite`): transactions, indexes, schema migrations,
  crash-safe by default, single-file local store, modest dependency weight.
- **B. Append-only JSONL**: framing + recovery, simple and inspectable; needs
  rotation and materialised projections for query performance at history scale.
- **C. Genuinely columnar Parquet**: right for analytical exports; wrong as the
  mutable operational store (append semantics, no transactions).
- **Hybrid** (e.g. SQLite operational + Parquet/JSONL export) only if justified.

## Decision

**Undecided — leaning SQLite (A) as operational store.** The current engine is
hardened in place (silent-failure removal, corruption detection, explicit
flush semantics) as an interim measure, but is not the target.

## Evidence required to decide

1. Measured query/load behaviour of representative inventory sizes (10k–100k
   assets, full history) under each candidate (benchmarks, Phase 2).
2. Corruption/recovery test results: crash mid-append, torn writes, partial
   flush (failure-injection tests, required before migration lands).
3. Dependency weight and cross-platform build cost comparison.
4. Migration path verification: lossless 1.x Parquet → target, with record-count
   verification and quarantine of unknown/malformed rows.

## Consequences

- Until accepted: no schema rewrites, no engine swaps; hardening of the existing
  engine proceeds under the "fails visibly" invariant.
- On acceptance: ADR updated with decision + consequences; migration tooling and
  rollback story land before any engine removal (`docs/migration-v2.md`).
- `balebos-storage-review` skill treats silent data loss as blocking.
