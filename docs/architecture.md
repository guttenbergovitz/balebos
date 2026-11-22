# Balebos architecture

Target architecture for Balebos 2.0. The implementation migrates incrementally
towards this structure; the map below describes responsibilities, not a command to
create empty modules. The current codebase lives in a transitional layout
(`app/ cli/ domain/ hosting/ inventory/ org/ storage/ tui/`) mapped onto these
responsibilities.

## Principles

1. **Evidence before interpretation.** Observations are facts about an inspection
   attempt; findings are deterministic derived assessments; assessment never
   rewrites observation.
2. **Unknown is not healthy.** Success, failure, skipped, unsupported, incomplete
   and unverified are distinct outcomes.
3. **Local-first.** No server, broker or central store; file-based persistence with
   explicit commit semantics.
4. **Composition over reinvention.** External scanners (subfinder, dnsx, httpx,
   RDAP clients) sit behind explicit adapters with bounded, cancellable execution.
5. **Determinism.** Identical persisted observations + rules produce identical
   assessments; scan metadata and presentation stay out of evaluation inputs.

## Module map

```text
cli, tui            → operator interfaces; no independent business rules
app                 → application services coordinating workflows
discovery/          → scope validation, discovery runner, tool adapters
inspection/         → dns, http, tls, rdap, asn, fingerprint adapters
domain/             → asset identity, observation, snapshot, finding, coverage
drift/              → snapshot comparison and change classification
assessment/        → transparent rules over persisted evidence
inventory/          → registry and history views over storage
storage/            → local persistence, versioning, migration, recovery
export/             → versioned machine-readable exports (JSONL/JSON/CSV)
```

## Dependency direction

```
cli, tui  →  app  →  domain, drift, assessment  →  storage
                  ↗
   discovery, inspection (adapters produce domain observations)
```

- `domain` depends on no other layer.
- Storage implements persistence contracts; nothing domain-level knows Parquet,
  SQLite or JSONL specifics.
- Discovery/inspection adapters return typed domain observations and capture tool
  identity, version, exit status and execution errors.
- CLI and TUI invoke the same application services and therefore agree on results.

## Persistence

SQLite is the authoritative operational store (ADR 0012): append-only
observations, transactional batches, indexed queries, reliable recovery,
single-process ownership. Current state is a query-time projection; history
is never edited.

Parquet survives as the sealed evidence archive only (ADR 0013): immutable
segments produced by an explicit archive step from committed observations,
with SHA-256 manifests and repeatable verification (ADR 0014). Sealed
segments are never rewritten. Hash chaining and remote WORM destinations
are recorded future options (ADR 0015, ADR 0016). The `parquet` crate is
used directly behind an optional feature; no DataFrame engine ships in the
default build (ADR 0017). Migration from legacy per-type Parquet files is
verify-don't-trust with rollback (ADR 0018).

Durability, integrity and immutability are distinct properties: SQLite
transactions give durability and consistency; SHA-256 manifests give
integrity against a trusted digest; only an external WORM policy can
approach immutability, under an explicit threat model.

## Subprocess execution

Explicit `std::process::Command` arguments only — no shell strings. Structured
JSON output parsed in preference to human-readable text. Every scan carries
documented, enforceable bounds: timeouts, concurrency, retries, scope, output
size and cancellation. Timed-out children are killed and reaped, including
descendants where supported. No unbounded spawning or retry storms. Private and
link-local addresses from untrusted discovery results are not probed unless an
explicit internal-network mode is enabled.

## Findings

Rules are transparent, deterministic and testable, operating on persisted
evidence. A finding links to its supporting observations and can answer: which
rule, which evidence, what is missing, was the scan complete, why this severity.
Lifecycle states (open, acknowledged, resolved, reopened, suppressed) are explicit
transitions with history; current state is never derived from an unordered event
bag. Arbitrary aggregate risk scores are avoided; explainable factors are kept.
