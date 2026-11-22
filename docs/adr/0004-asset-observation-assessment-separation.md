# 0004 — Asset, observation and assessment separation

**Status:** Accepted

## Context

1.x stores everything as `InventoryItem`/`ItemType` variants and JSON blobs:
assets, observations, findings and organisational records share one enum and one
discriminator. The split between "what exists" (inventory) and "what was seen"
(observations) exists conceptually in `documentation/architecture.md` but is not
enforced in types, and "what we conclude" (assessment) is entangled with both.
Consequences observed in the audit: unknown record types coerced to `Domain`,
observations carrying implicit meaning, findings with no evidence links.

## Decision

Three distinct concepts, enforced in the type system:

- **Asset identity** — the thing tracked, with stable, explicit identity rules.
- **Observation** — what an inspection attempt found at a moment, with full
  provenance (target, inspection kind, timestamp, source/adapter, result,
  completeness, error if any). Parse failure is distinct from a valid
  observation containing an error.
- **Assessment** — deterministic rules over persisted observations producing
  findings. Assessment never rewrites observations; findings link to supporting
  evidence.

Newtyped IDs (`AssetId`, `ScanId`, `ObservationId`, `FindingId`, `SnapshotId`)
and enums (`AssetKind`, `ObservationOutcome`, `ScanStatus`, `ScanCoverage`)
replace stringly-typed state. Validated newtypes appear at subsystem boundaries;
open-ended external tool metadata stays a typed-where-stable / map-where-genuinely
open-ended structure.

## Consequences

- Phase 2 introduces the domain types incrementally alongside existing ones;
  the 1.x `ItemType` storage discriminator persists only inside the storage
  adapter until the persistence migration (ADR 0007).
- No fabricated confidence scores: provenance and completeness fields carry the
  trust information.
- Every drift conclusion and finding becomes explainable by construction
  (required by ADR 0005 and the assessment engine redesign).
