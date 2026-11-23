# 0026 — External network service discovery

**Status:** Accepted (core model & offline ingestion); Proposed (optional authorised active protocol verification)

## Context

Balebos previously modelled domains, FQDNs, IP addresses, ASNs, TLS certificates, HTTP endpoints, and providers. Publicly exposed network services (databases, message brokers, SSH, SMTP, management interfaces) could not be represented independently of hostnames, nor could Balebos distinguish a bare TCP port number from a corroborated application protocol.

## Decision

1. **First-class `NetworkService` entity (`src/domain/service.rs`, `src/domain/relationship.rs`)**: Represent network services independently of hostnames using canonical socket identity (`ip:port/transport` or `host:port/transport` when only a hostname is known).
2. **Port ≠ protocol**: A port number (e.g. `5432/tcp`, `6379/tcp`, `9200/tcp`) produces a *protocol hypothesis* (`ProtocolIdentification::PortHypothesis`), never an authoritative protocol claim. Banner/fingerprint/response evidence is recorded explicitly as `ProtocolIdentification::IndexedBanner` or `ProtocolIdentification::ObservedProtocol`, preserving source and uncertainty.
3. **Immutable `ServiceObservation` persistence (`ItemType::ServiceObservation`)**: Store observed endpoint, transport, optional application protocol, service state (`Open`, `Filtered`, `Closed`, `HistoricalOnly`, `Unknown`), identification method, source, provider timestamp (`source_recorded_at`), Balebos `ingested_at`, supporting evidence, and error/uncertainty state.
4. **No authentication or data extraction**: Database and management interface discovery records exposure and fingerprints only; credential guessing, data queries, and unrestricted port scanning remain prohibited.

## Alternatives considered

- **Storing ports as string tags on IP or Domain items**: Rejected because an IP exposes multiple independent services with distinct timestamps, fingerprints, and lifecycles.
- **Treating IANA well-known ports as confirmed protocols**: Rejected because services routinely run on non-standard ports or present deceptive banners.

## Consequences and risks

- **Consequences**: Services survive restarts in SQLite, participate in `exposes_service` / `speaks_protocol` / `presents_certificate` / `shares_ssh_host_key` relationships, and appear in CLI/TUI investigation views.
- **Risks**: Multiple providers reporting the same `ip:port/tcp` can create duplicate observations unless correlated by canonical socket identity; mitigated via deterministic deduplication in relationship projection and candidate correlation.

## Verification criteria

- Deterministic offline tests prove `NetworkService` independence from domains, port-5432 non-PostgreSQL separation, historical service expiry, and SQLite round-trip persistence.
