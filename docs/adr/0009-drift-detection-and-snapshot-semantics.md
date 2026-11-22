# 0009 — Drift detection and snapshot semantics

**Status:** Proposed

## Context

1.x `diff` compares domain inventories over a time window (new/missing names
only). It cannot distinguish a verified disappearance from an incomplete scan,
does not detect within-asset drift (DNS change, HTTP behaviour change, TLS
change, redirect-chain change, attribution or fingerprint change), and is
vulnerable to transient DNS/CDN/load-balancing noise. The brief makes historical
comparison a central capability.

## Decision (draft — pending Phase 4 evidence)

- A **snapshot** is an immutable set of observations with explicit coverage
  (ADR 0005): a `SnapshotId`, the scan identity, and the outcome classification
  per target.
- Comparison happens between snapshots (or between a snapshot and the live
  latest state), classifying changes into a fixed taxonomy: newly discovered,
  not observed (incomplete scan), verified disappearance (method-limited),
  DNS change, HTTP behaviour change, TLS change, redirect-chain change,
  attribution (ASN/hosting) change, fingerprint change, inspection failure,
  insufficient evidence.
- Transient noise is damped by documented rules (e.g. a change is reported when
  observed consistently across N consecutive snapshots, or immediately when the
  change class is deterministic like TLS certificate replacement); every
  reported drift links to the observations that support it.
- Historical observations are never modified when assets are renamed,
  reclassified or acknowledged; identity mapping is a projection, not an edit.

## Evidence required

1. Deterministic fixtures for each drift class (unit tests, Phase 4).
2. Behaviour with real-world noisy targets (CDN rotation, wildcard DNS) in the
   explicitly-invoked live suite, informing damping defaults.
3. CLI/TUI agreement tests on the same snapshot pairs.

## Consequences

- Landed together with the assessment engine (findings consume drift output).
- Arbitrary aggregate risk scores are not part of this design; explainable
  per-change evidence is (brief §11).
