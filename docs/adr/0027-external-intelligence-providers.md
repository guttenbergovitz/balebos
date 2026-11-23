# 0027 — External intelligence providers (Censys, Shodan, urlscan.io)

**Status:** Accepted (provider-neutral contract + offline Censys/Shodan/urlscan.io adapters & fixtures); Proposed (live credentialed API transport)

## Context

Enumerating externally observable services and website resource dependencies without internet-wide scanning requires passive intelligence providers such as Censys, Shodan, and urlscan.io. Coupling domain types to a single vendor API or requiring paid subscriptions would violate Balebos' local-first constraint (ADR 0001).

## Decision

1. **Provider-neutral contracts (`src/app/external_services.rs`, `src/app/references.rs`)**: Core domain and correlation logic consumes `ServiceObservation` and `ApplicationReferenceObservation`, never raw vendor JSON schemas.
2. **Offline-first adapters with optional credentials**: Implement deterministic JSON/JSONL parsers and fixture-backed adapters for Censys host records (`censys-hosts-v2`), Shodan host records (`shodan-host-v1`), and urlscan.io indexed results (`urlscan-v1`). Paid API credentials remain optional; live paid endpoints are documented as unavailable without operator credentials.
3. **Timestamp and provenance preservation**: Every imported observation preserves `source_recorded_at` (the provider's scan timestamp) separately from `ingested_at` and never presents stale external index entries as live Balebos checks.
4. **urlscan.io submission boundary**: Separate passive lookup of already-indexed public results from newly requested scans; internal or out-of-scope URLs are never automatically submitted to third-party scanners.

## Alternatives considered

- **Hard-coding Shodan or Censys structs in `src/domain/`**: Rejected because vendor schema changes and multi-source correlation would pollute the domain layer.
- **Requiring live API keys for CI**: Rejected by offline deterministic test policy.

## Consequences and risks

- **Consequences**: Multi-source service and website dependency data can be ingested, deduplicated, and audited offline.
- **Risks**: Provider banners may be spoofed or stale; mitigated by tagging provider claims as `Indexed` and preserving provider timestamps and conflict states.

## Verification criteria

- Fixture tests verify Censys, Shodan, and urlscan.io parsing, stale timestamp retention, multi-source conflict preservation, and cross-source deduplication.
