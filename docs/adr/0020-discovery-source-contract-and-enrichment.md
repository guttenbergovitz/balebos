# 0020 — Discovery source contract and external enrichment

**Status:** Accepted (CT + provider-neutral Passive DNS contract); Proposed (paid live Passive DNS providers)

## Context

Expanding discovery beyond active single-point probing (`subfinder`, `dnsx`, `httpx`) requires passive intelligence sources such as Certificate Transparency (CT) and Passive DNS. Implementing each provider as an ad-hoc subsystem risks mixing external claims (`source_recorded_at`) with direct measurements (`observed_at` / `ingested_at`) and conflating provider failures with negative asset facts.

## Decision

1. **Minimal source descriptor contract (`src/app/sources.rs`)**: Every discovery source declares its `id`, `api_version`, `operation_mode` (`Passive` / `Active`), `requires_credentials`, `scope_required`, and documented limitations.
2. **Timestamp separation**: External records preserve `source_recorded_at` separately from `ingested_at` and direct `observed_at` measurements. Ingestion time is never treated as historical observation time.
3. **Certificate Transparency**: Implemented via a bounded `crt.sh` JSON adapter (`CertificateTransparencySource`) supporting offline JSON import/fixtures and opt-in bounded queries. Wildcard SANs and multi-domain certificates produce candidates, never automatic ownership claims.
4. **Passive DNS**: Implemented as a provider-neutral contract (`PassiveDnsSource`) with offline JSONL/JSON ingestion and deterministic fixture tests. Paid external APIs remain optional and unbundled.
5. **Failure semantics**: Provider errors or timeouts never imply that an asset or relationship is absent.

## Consequences and trade-offs

- Balebos remains local-first with zero mandatory cloud dependencies or paid credentials.
- External enrichment stays attributable, bounded and testable offline in CI.

## Rejected alternatives

- Bundling provider-specific SDKs or requiring paid API keys in core workflows.
- Storing raw provider JSON directly inside `domain` types.

## Verification criteria

- Fixture tests prove timestamp separation, wildcard/multi-SAN handling, and failure isolation.
