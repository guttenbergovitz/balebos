# 0014 — Evidence segment sealing and verification

**Status:** Accepted

## Decision

Archival proceeds in explicit, idempotent stages; only the last marks
evidence as archived.

1. **Selection** (deterministic): a committed, reproducible set of
   observations (scan ids or explicit observation ids / record boundaries).
   Nothing is omitted silently; the selection is recorded.
2. **Sealing**: one immutable Parquet segment per selection; the raw file
   bytes are hashed with SHA-256. A manifest records: archive id, schema
   version, segment id, selection (scan/observation boundaries), record
   count, segment SHA-256, creation timestamp, source/schema metadata, and
   the previous sealed segment hash when chaining (ADR 0015).
   Physical-file hashing is used; a canonical per-record representation is
   only added if independent record verification is ever required — the
   two are never conflated.
3. **Publication** (crash-safe): segment and manifest written to temporary
   files, fsynced, verified (re-read, size and hash check), then renamed
   into place. An archive is committed only when both files are durably
   published and verified.
4. **Marking**: only after publication, SQLite rows are marked archived —
   never deleted at this stage. A failed archive invalidates nothing that
   was already committed operationally.

**Verification** (`archive verify`): re-hash segments, check manifests,
record counts, schema versions, chain continuity, missing/duplicate
segments, and broken evidence references. **Restoration**: verified
segments re-import into SQLite with count/identity verification.

## Context

The evidence lifecycle (collect → archive → seal → verify → optionally
remote) needs unambiguous, testable guarantees at each stage; the 1.x
storage had none. Idempotency and crash-safety are required because archive
operations will be re-run after failures (crashes, power loss, full disk,
interrupted uploads).

## Consequences

- Archive operations are restartable; duplicates are detected via segment
  id and manifest, not by heuristics.
- Failure-injection tests are mandatory for the publication path (torn
  writes, missing manifests, corrupt segments, interrupted re-runs).
- The storage-review skill treats "source deleted before verified archive"
  as blocking.
