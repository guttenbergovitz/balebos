# ADR-001: Replace ephemeral Issues with persisted Findings

Status: accepted, 2026-04-12

## Context

The current `check.rs` module runs six hardcoded checks against inventory and observations. Each check produces `Issue` structs with a severity (Warning/Critical) and a message string. These issues are printed to stdout and discarded — they don't survive beyond the CLI invocation.

This means:
- You can't query "what was wrong last week"
- You can't acknowledge or resolve an issue
- You can't track whether things are getting better or worse
- The report module (`report.rs`) counts issues by parsing message strings (fragile)
- There's no audit trail of what the system detected

## Decision

Replace `Issue` with a `Finding` entity that is persisted to storage as an append-only record, like every other record type in Balebos.

### Finding struct

```rust
pub struct Finding {
    pub id: String,                          // UUID
    pub severity: FindingSeverity,           // Info, Low, Medium, High, Critical
    pub category: FindingCategory,           // Security, Compliance, Financial, Lifecycle, Operational, Governance
    pub check_name: String,                  // which check produced this (e.g. "tls_expiry", "staleness")
    pub target: String,                      // affected asset
    pub message: String,                     // human-readable description
    pub created_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
}
```

### Severity levels

Expand from 2 (Warning, Critical) to 5: Info, Low, Medium, High, Critical.

### Categories

Security, Compliance, Financial, Lifecycle, Operational, Governance — matching the question taxonomy from the big picture document.

### Storage

Findings are stored as `ItemType::Finding` records in the append-only event log (JSONL). Lifecycle transitions (acknowledge, resolve) are separate records that reference the finding ID.

### CLI surface

- `balebos check` — runs checks, persists new findings, prints summary
- `balebos findings` — lists open (unresolved) findings
- `balebos findings --all` — lists all findings including resolved
- `balebos finding ack <id>` — acknowledges a finding

### Deduplication

When `balebos check` runs, it should not create duplicate findings for the same issue. A finding is considered duplicate if it has the same `check_name` + `target` combination and is still open (not resolved). Duplicate detection reads existing open findings before creating new ones.

## Consequences

- Findings become queryable, trackable, and auditable
- The report module can count findings by querying storage instead of parsing strings
- Exit codes can be based on finding severity (critical findings → non-zero exit)
- Future phases (API, dashboard, AI escalation) have a concrete entity to work with
- Slight increase in storage volume (findings are persisted), but at expected scale this is negligible
- `reader.rs` gets one new branch in the match statement for `ItemType::Finding`

## What this does NOT change

- The checks themselves (staleness, TLS expiry, etc.) remain the same logic
- Storage architecture stays the same (append-only JSONL, optional DuckDB)
- No new dependencies
