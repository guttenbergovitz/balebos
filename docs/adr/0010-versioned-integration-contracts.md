# 0010 — Versioned machine-readable integration contracts

**Status:** Proposed

## Context

1.x exports JSONL/CSV/Parquet ad hoc from storage internals, with no schema
versioning, no stability guarantee and export paths that silently drop
unparseable records. External consumers (CMDB sync, reporting, CI gates)
currently couple to implementation details.

## Decision (draft — pending Phase 6)

- Balebos exposes versioned, machine-readable exports as the **only** supported
  integration surface: assets, observations, findings and scan coverage as
  distinct, documented schema versions (e.g. `balebos.export/v1`).
- stdout stays reserved for results, stderr for diagnostics; structured output
  never mixes with progress messages.
- Exit codes are part of the contract: execution failure (Balebos failed) is
  distinct from operational findings (broken infrastructure found); partial
  scans remain identifiable in structured output.
- Legacy 1.x JSONL import remains supported as an input contract for migration.
- External assessments (e.g. vulnerability reports from Angry Turnip) may be
  referenced only via contracts preserving original sources and uncertainty;
  no re-interpretation inside Balebos.

## Evidence required

1. A concrete consumer exercise (shell script + CI gate) proving the export is
   sufficient without internal access (Phase 6, Workflow G).
2. Schema evolution policy (additive-only within a major version) documented
   and enforced in review.

## Consequences

- Export modules move behind the `export/` responsibility with fixture-backed
  round-trip tests (import/export stability).
- The web dashboard becomes one consumer of the contract, not a special case.
