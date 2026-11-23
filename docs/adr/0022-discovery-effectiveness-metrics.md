# 0022 — Discovery effectiveness measurement framework

**Status:** Accepted

## Context

Counting raw observations or graph edges does not measure whether Balebos discovers previously unknown, relevant infrastructure or avoids false attribution.

## Decision

Implement `DiscoveryMetricsReport` (`src/app/metrics.rs`) measuring:
1. **Discovery yield & marginal gain**: raw candidates, unique unknown candidates not already in inventory, and per-source unique contribution (comparing baseline scanners against CT, Passive DNS, and local pivot correlation).
2. **Evidence quality**: percentage of relationships with resolvable observation IDs, current vs historical/uncertain breakdown.
3. **Candidate precision & review coverage**: operator-reviewed `Confirmed` vs `Rejected` counts, explicitly keeping `Unreviewed` and `Deferred` separate from false positives.
4. **Operational efficiency**: query/projection latency (`elapsed_ms`) without introducing an external observability stack.

## Consequences and trade-offs

- Metrics that require ground truth (precision/recall) are strictly separated from automatically computable metrics (yield, overlap, evidence traceability, latency).

## Rejected alternatives

- Single opaque "discovery quality" score.
- Counting unreviewed candidates as false positives.

## Verification criteria

- Deterministic 14-scenario offline benchmark fixture suite verifies yield, marginal gain, precision accounting, and latency reporting.
