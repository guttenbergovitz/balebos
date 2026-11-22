---
name: balebos-storage-review
description: >-
  Storage subsystem review for crash consistency, corruption handling, schema
  compatibility, migration safety and recovery. Silent data loss is a blocking
  issue. Use for any change touching src/storage, persistence formats or
  migration tooling.
---

# Balebos Storage Review

Treat silent data loss as **blocking**. Review actual code and tests; output
numbered findings with `path:line`, severity and a concrete fix. Checklist
follows the persistence requirements in ADR 0007.

## Checklist

1. **Crash consistency.** Every durable write committed via explicit
   flush/commit semantics (no destructor-only durability)? Writes atomic
   (temp file + fsync + rename, or transactional)? Crash mid-write leaves the
   previous state intact?

2. **Atomicity.** Appends never read-modify-write whole files non-atomally?
   Concurrent writers excluded or serialised safely?

3. **Buffer handling.** Buffered records survive error paths? Buffer drained
   only after successful commit? Flush failure visible to the caller, not
   swallowed?

4. **Schema compatibility.** Versioned schemas? Readers tolerate old versions
   explicitly? Unknown fields/types quarantined or reported — never coerced to
   unrelated defaults (e.g. unknown record type → `Domain` is blocking)?

5. **Migration safety.** Migrations run inside transactions where the engine
   supports them? Pre-migration backup required for destructive transforms?
   Record-count verification before/after? Rollback documented?

6. **Corruption detection.** Malformed records surfaced with counts and
   identities, not silently skipped? A torn tail is detectable? Operator has a
   documented repair path?

7. **History preservation.** No in-place updates of historical observations?
   Rename/reclassify/acknowledge operations are projections or new records,
   never edits of the past?

8. **Query correctness.** Readers bounded (no full-history load for targeted
   queries) or the bound explicitly documented? No silent dedup that would
   erase legitimately repeated observations?

9. **Recovery testing.** Failure-injection tests exist: interrupted write,
   torn file, corrupt record, partial flush, restart recovery. Missing recovery
   tests for new durability claims is should-fix; for claimed crash-safety it
   is blocking.

10. **Concurrent access assumptions.** Single-process assumption documented?
    Locking honest about its scope (in-process vs inter-process)?

## Output format

```text
Dimension: Corruption detection
- [blocking] src/storage/parquet.rs:141 unknown item_type coerced to
  ItemType::Domain — silent classification corruption. Quarantine or report.
...
Verdict: PASS / PASS WITH NITS / CHANGES REQUIRED (blocking findings present)
```
