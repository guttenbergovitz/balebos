# 0005 — Scan completeness and uncertainty semantics

**Status:** Accepted

## Context

1.x has no explicit concept of scan coverage. `diff` compares inventories and
treats "not seen this time" as "missing"; a failed probe pass can therefore imply
that known assets disappeared, and a partial batch run is indistinguishable from
a complete one. This violates the product invariant "unknown is not healthy".

## Decision

Every scan is bounded by documented, enforceable limits (time, concurrency,
retries, scope, output size, cancellation) and records its **coverage**:
which targets were planned, which were attempted, which succeeded, failed,
were skipped or unsupported, and which were never attempted. Outcomes are
explicit and never conflated:

- successful observation ≠ failed observation ≠ skipped ≠ unsupported ≠
  incomplete ≠ unverified ≠ confirmed absent.
- A failed discovery pass changes nothing about asset existence: previously
  known assets remain known with "not observed this scan".
- Confirmed absence is claimed only where the observation method genuinely
  supports it (e.g. authoritative NXDOMAIN for a previously resolved name), and
  is recorded as its own observation outcome.
- Partial scans are labelled partial everywhere they surface (CLI, TUI, exports).

Determinism: given identical persisted observations, baseline and rules,
assessments are identical. Scan timestamps and execution metadata are not
evaluation inputs.

## Consequences

- Phase 3 (discovery/inspection refactor) must record coverage per scan;
  Phase 4 (snapshots/drift) consumes it to distinguish "not observed" from
  "verified gone".
- Drift classification gains an "insufficient evidence" category (ADR 0009).
- TUI and CLI must both surface coverage; the TUI work in Phase 5 includes a
  visible incomplete-coverage indicator.
