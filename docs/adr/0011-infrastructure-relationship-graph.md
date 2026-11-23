# 0011 — Infrastructure relationship graph

**Status:** Accepted (Phase B–D implemented as deterministic projections)

## Context

A future development phase may introduce an evidence-backed infrastructure
relationship graph, inspired by the exploration capabilities of Netlas and
Maltego. The graph would represent entities such as domains, FQDNs, IP
addresses, ASNs, certificates and endpoints, together with typed,
time-dependent relationships between them.

This direction is recorded **now**, while the 2.0 domain model is being
hardened, so that current design decisions keep the door open instead of
closing it. It is an extension of the existing product, not a replacement
for its core purpose, and it must not interrupt or derail the ongoing
migration.

## Decision (implemented incrementally without a graph engine)

The following constraints bind the relationship model and queries:

- **Stable entity identities.** Asset identity rules (ADR 0004) must remain
  stable and explicit; identity is never derived from mutable attributes.
- **Provenance and timestamps preserved.** Every observation keeps its
  source, time and completeness (ADRs 0004, 0005); relationships will be
  built from this evidence, not from re-probed guesses.
- **Technical relationships vs organisational attribution stay separate.**
  Observed technical linkage (same IP, same certificate) is evidence; it is
  never conflated with inferred ownership or governance (ADRs 0002, 0008).
- **History is immutable.** Relationships may change or expire; the evidence
  supporting them is never destroyed or edited retroactively.
- **No common-ownership assumption.** Shared infrastructure (shared IP, CDN,
  certificate) does not establish common ownership; any such inference would
  be a separate, explicitly labelled assessment.
- **No graph database, graph UI or generic transformation engine at this
  stage.** The local-first constraint (ADR 0001) stands; the storage
  decision (ADR 0007) must not be driven by graph ambitions.

## Evidence required to proceed (after 2.0 foundations are reliable)

1. Concrete operator workflows the graph would improve (exploration,
   incident scoping, footprint review), stated as scenarios.
2. Demonstrated volume of cross-entity observations in real inventories,
   informing whether adjacency can live in the chosen store without a
  dedicated engine.
3. A typed relationship model sketch (entity kinds, edge kinds, validity
   intervals) reviewed against ADR 0004 types.

## Consequences

- Current phases (domain hardening, discovery/inspection, snapshots/drift)
  proceed unchanged; nothing in them may take a dependency on graph concepts.
- The constraints above act as design review criteria: any 2.0 change that
  violates them (unstable identity, destroyed provenance, conflated
  attribution) is a blocking finding in architecture review.
