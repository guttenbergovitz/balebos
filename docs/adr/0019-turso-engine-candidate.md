# 0019 — Turso as a candidate operational engine (tracked, not adopted)

**Status:** Proposed

## Context

Balebos runs on SQLite via `rusqlite` (bundled) since ADR 0012. Turso
(github.com/tursodatabase/turso) is an in-process SQL database **written
in Rust**, compatible with SQLite at the SQL dialect, file-format and C
API levels, positioned as the successor to libSQL. It offers `BEGIN
CONCURRENT` (MVCC), change data capture, vector search and async I/O.

Motivations to consider it: pure-Rust dependency graph (no C toolchain),
potential fit for future evidence-graph queries (ADR 0011), and file
format compatibility that would keep the ADR 0018 migration path cheap.

Reasons not to adopt now:

- **Pre-1.0.** The project states compatibility is "not at 100% yet"
  (COMPAT.md), declares some features experimental — including
  multi-process WAL coordination — and recommends independent backups
  until 1.0. Balebos's engine underwrites an evidence store (ADR 0014);
  durability edge cases are the exact risk class our failure-injection
  tests exist to exclude.
- **Async-first API.** The Rust bindings are async (`build().await`,
  `query().await`). Balebos's discipline is synchronous, bounded
  operations (brief §10); adopting Turso today would pull an async
  runtime into a tree deliberately kept small (ADR 0017).
- **No concrete rusqlite defect.** The build-time problem that motivated
  dependency scrutiny is solved (Phase E: 4m 01s → 44.1s, 80 → 6.7 MiB);
  bundled SQLite compiles in seconds and is among the most tested
  software in existence.

## Decision (draft)

SQLite via rusqlite remains the operational engine. Turso is the tracked
successor candidate.

## Evidence required to proceed (all must hold)

1. Turso 1.0 released with full SQLite compatibility declared.
2. A synchronous (or trivially bridgeable) Rust API — no async runtime
   requirement in the default build.
3. The storage failure-injection suite (interrupted writes, crash
   recovery, archive protocol tests) re-proven green against Turso.
4. A concrete deficiency in rusqlite for Balebos workloads, or a
   measured maintenance win, documented with numbers.

## Consequences

- No code changes now; this ADR exists so the option is weighed against
  evidence rather than enthusiasm.
- Dependency policy (ADR 0017) already covers any future swap: measured
  justification, ADR supersession, migration verified per ADR 0018
  semantics.
