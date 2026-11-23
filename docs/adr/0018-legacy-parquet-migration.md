# 0018 — Legacy Parquet migration

**Status:** Accepted (plan; execution in Phase C)

## Decision

Migration from 1.x-era per-type Parquet files (`*.parquet` containing
`raw_json` rows) to SQLite (ADR 0012) follows a verify-don't-trust path:

1. **Discover** all `*.parquet` files in the legacy data directory.
2. **Read** every row; classify each as: supported record (known
   `item_type`, valid schema), unknown-type record, malformed record.
3. **Preserve unrecognised data**: unknown/malformed rows are quarantined
   verbatim (a `quarantine` table mirroring the interim engine's
   quarantine.parquet semantics) — never coerced to a valid type, never
   silently skipped.
4. **Import** supported records transactionally into SQLite, preserving
   ids, timestamps and order; observation rows stay append-only.
5. **Verify**: record counts per type before vs after match; quarantine
   counts match; spot-check identities and timestamps.
6. **Roll back**: the legacy directory is only renamed (e.g. to
   `data/parquet.migrated-<timestamp>`), never deleted; rollback = point
   the engine back at it and remove the SQLite file.
7. Only after independent verification does the default engine flip.

Findings, tags and user-maintained metadata migrate with the same rules;
org-governance records migrate as data but their subsystem remains retired
per ADR 0008 (export path documented in `docs/migration-v2.md`).

## Context

The 1.x store contains real operator evidence. The 2.0 evidence-integrity
invariants (no silent loss, no fabricated defaults) apply to migration with
full force — a migration that loses history would defeat the product.

## Consequences

- Migration tooling ships before the Parquet engine leaves the default
  build; legacy read support stays until Phase C is verified.
- Failure modes (partial import, corrupt legacy file, restart mid-import)
  are failure-injection tested; the tool is idempotent where re-runnable.
