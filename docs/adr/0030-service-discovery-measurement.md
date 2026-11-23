# 0030 — Service discovery measurement

**Status:** Accepted

## Context

ADR 0022 established discovery metrics for domains, CT, Passive DNS, and local pivots. Expanding into network services and application references requires measuring service discovery yield, per-source contribution across the new sources, verification quality, and per-category reviewed precision.

## Decision

Extend `DiscoveryMetricsReport` (`src/app/metrics.rs`) to report:
1. **Service & reference yield**: Unique `NetworkService` endpoints, protocol identifications, management interfaces, API endpoints, and storage references.
2. **Per-source unique contribution**: Track deduplicated unique candidates contributed by Censys, Shodan, urlscan.io, JavaScript analysis, OpenAPI, DNS SRV/TXT, and public repository references.
3. **Evidence & verification quality**: Traceable evidence percentage, independently verified count/rate, historical-only count, uncertain count, and operator-reviewed count.
4. **Per-category reviewed precision**: Compute reviewed precision separately for network services, database candidates, management interfaces, APIs, storage references, and inferred dependencies, never counting `Unreviewed` or `Deferred` as false positives.
5. **Operational timings**: Report projection and evaluation elapsed milliseconds without fabricating accuracy metrics.

## Alternatives considered

- **Aggregating all candidate types into a single precision number**: Rejected because noisy storage or JS references would obscure high-precision service discoveries.

## Consequences and risks

- **Consequences**: Measurable comparison across baseline DNS/HTTP discovery, external service indexes, and application reference discovery.

## Verification criteria

- 22-scenario offline benchmark suite exercises service yield, deduplication across sources, historical-only accounting, and per-category precision.
