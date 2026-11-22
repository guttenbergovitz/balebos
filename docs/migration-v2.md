# Migration guide: Balebos 1.x → 2.0

Status: living document. Updated as each disposition is implemented.

## Principles

- Historical observations and user-maintained metadata are never silently discarded.
- Migration correctness matters more than preserving implementation complexity.
- Each capability is classified: **retained unchanged**, **retained with changed
  interface**, **migrated**, **deprecated** or **removed**.
- Rollback options are documented per change; where rollback is impossible
  (destructive transforms), a pre-migration backup is required.

## Breaking-change surface (planned)

| Area | 1.x | 2.0 | Classification |
|---|---|---|---|
| Persistence | Parquet files with JSON-string column, per `ItemType` | decision pending (ADR 0007); migration tool will convert 1.x Parquet | migrated |
| Org governance (`org *` commands, Brand→Company→Project→Person, role/contractor assignments) | full subsystem | lightweight local metadata (tags, notes, importance, owner reference) | removed (data preserved + exportable) |
| Risk scoring (`risk report`) | weighted aggregate score | explainable finding factors without arbitrary aggregate | removed (interface), findings retained |
| `check` / `report` (ephemeral) | printed issues | persisted findings with evidence links | migrated |
| CLI | see `documentation/guides/usage.md` | largely compatible; output discipline hardened (stdout results / stderr diagnostics) | retained with changed interface |
| Exports | JSONL/CSV/Parquet ad hoc | versioned machine-readable contracts (ADR 0010) | retained with changed interface |
| Exit codes | rich set defined in `src/exit_codes.rs` | documented, enforced consistently | retained unchanged (verified) |

## Data migration

1.x stores per-type Parquet files (`domain.parquet`, `finding.parquet`, …) whose
rows are JSON strings. Migration to SQLite follows ADR 0018 (verify-don't-trust):

1. **Discover** all legacy `*.parquet` files.
2. **Read and classify** every row: supported record / unknown type / malformed.
3. **Quarantine** unknown and malformed rows verbatim in SQLite (never coerced,
   never dropped).
4. **Import** supported records transactionally, preserving ids, timestamps
   and order.
5. **Verify**: per-type record counts before vs after; quarantine counts;
   identity/timestamp spot checks.
6. **Roll back**: legacy directory is renamed, never deleted; rollback =
   point the engine at the old directory and remove the SQLite file.

Migration tooling ships and is failure-tested before the Parquet engine
leaves the default build.

## Removed features and their data

- **Organisational records** (`org_*.parquet`): exported to JSONL before removal;
  lightweight local replacements (tags, notes, importance, owner string) accept
  a one-time import of the essentials (asset → owner/project name as plain text).
- **Risk weights**: no data of lasting value; findings and their evidence are retained.

## Timeline

Phases follow `docs/architecture.md` and the ADRs; no calendar commitments are
made here. Each phase lands with tests and a checkpoint report.
