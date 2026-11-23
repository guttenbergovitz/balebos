# 0021 — Temporal relationship semantics and candidate attribution

**Status:** Accepted

## Context

Early relationship projections marked an edge `is_current = true` whenever its last positive observation was never replaced by another positive observation of the same kind, allowed non-overlapping historical observations to appear as concurrent shared infrastructure, promoted correlated CDN attributes (`IP + TLS cert`) to `StrongCandidate`, and treated inventory membership as active-probing scope.

## Decision

1. **Freshness on valid re-observation**: Every valid DNS observation advances the currency watermark for all DNS relationship kinds (`ResolvesTo`, `AliasesTo`, `UsesNameserver`, `RoutesMailTo`), and every successful HTTP observation (`status.is_some()`) advances `PresentsCertificate` and `RedirectsTo`. Failed probes never erase prior edges.
2. **Temporal overlap**: Shared-infrastructure correlation checks whether `[first_seen, last_seen]` intervals overlap (`concurrent: bool`), separating concurrent infrastructure sharing from historical-only coincidences.
3. **Signal independence**: Relationship signals are grouped into `SignalPlane`s (`NetworkAddress`, `TlsIdentity`, `DnsDelegation`, `HttpNavigation`). Shared IP + TLS behind a detected CDN/shared platform remains `WeakSharedClue`.
4. **Explicit scope**: Pivot discovery never promotes `records.items` into `approved_domains`. Out-of-scope candidates are surfaced passively for operator review (`Confirmed` / `Rejected` / `Deferred`).

## Consequences and trade-offs

- Fewer false positives on shared CDNs and stale DNS records.
- Operators must explicitly pass `--scope` when authorising pivot expansion.

## Rejected alternatives

- Uncalibrated numerical confidence scores.
- Treating first-seen..last-seen as proof of uninterrupted existence.

## Verification criteria

- Deterministic unit tests verify empty re-observation expiry, non-overlapping IP separation, CDN signal suppression, and out-of-scope protection.
