# 0012 — SQLite as authoritative operational storage

**Status:** Accepted

## Context

ADR 0007 (now superseded) documented the interim Parquet engine's defects:
JSON strings in a single column, whole-file rewrite on append, no
transactions, silent corruption paths. Phase A measurement (.specs/06)
confirmed the dependency structure: 424 packages, Polars pulled with full
defaults (tokio/hyper/reqwest baggage), used only as a JSON container.
Balebos needs transactional local persistence, indexed queries, reliable
recovery and append-only evidence semantics — properties of a database, not
a DataFrame engine.

## Decision

SQLite (via `rusqlite`, bundled) is the authoritative operational store for:
asset identities, inventory, observations, scan metadata and coverage,
findings and lifecycle, tags and local metadata, future graph nodes/edges,
and archive manifests/status.

- Append-only observation semantics: observations are INSERTed, never
  UPDATEd; current state is a query-time projection.
- One writer per process (single-user tool); WAL mode for reader
  concurrency; explicit transactions per batch.
- Durability, not immutability: SQLite gives transactional consistency and
  crash recovery. It does not protect against a privileged actor; that is
  the archive's job (ADR 0013/0014).
- Historical retention baseline: observations remain in SQLite until an
  archive segment is sealed and verified (ADR 0014); no removal before that.
- Parquet (1.x-era) files and Polars are removed from the normal execution
  path (Phase E); SQLite alone decides query results.

## Consequences

- `StorageEngine` gains a SQLite adapter; the trait stays the seam.
- A verified, reversible migration from legacy Parquet data is mandatory
  before the default engine flips (ADR 0018).
- Default build sheds Polars and its transitive graph; compile time and
  binary size drop (measured in Phase E against the Phase A baseline).
- Single-process assumption documented; no inter-process locking added.
