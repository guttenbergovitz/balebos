# 0013 — Parquet as immutable evidence archive (optional)

**Status:** Accepted

## Decision

Parquet remains in the product solely as the **sealed evidence archive**
format: immutable, self-describing batches of historical observations for
long-term retention, portability, compression and offline analytics.

- Sealed segments are never rewritten; new observations form new segments
  (ADR 0014 defines sealing and manifests).
- Segments are created by an explicit archive step from committed SQLite
  observations — archival is a deliberate, verifiable act, not a side
  effect of scanning.
- The `parquet` crate is used directly (Arrow types where justified) behind
  an optional `parquet-archive` feature; the default build does not compile
  it, and no DataFrame engine is included.
- S3 Object Lock / Glacier is an optional future destination for sealed
  segments (ADR 0016); remote storage is never required for operation.

## Context

Parquet's genuine value is archival: columnar compression, ecosystem
interoperability, suitability for batch analytics and long-term storage.
Its misuse was as a mutable operational store (ADR 0007). This decision
keeps the format and drops the misuse.

## Consequences

- Archive code lives apart from discovery/TUI concerns and speaks domain
  types, not storage internals.
- Reading archives is a restore/verify path, not the live query path.
- If the `parquet` crate's compile cost proves unjustifiable even as an
  optional feature, JSONL segments are the documented fallback (same
  manifest and hashing scheme); the decision point is Phase D measurement.
