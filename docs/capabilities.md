# Balebos Capability Inventory

Code-verified inventory of what Balebos measures, calculates, infers and plans (against `develop`).

## A. External Measurement Engines

| Capability | Engine | Location | Mode | Credentials | Provenance | CLI / TUI |
|---|---|---|---|---|---|---|
| Subdomain enumeration | `subfinder` (optional Chaos) | `src/app/probe/subfinder.rs` | Passive / OSINT | Optional API keys | `source: "subfinder"`, `discovered_at` | CLI + TUI |
| DNS inspection (A, AAAA, CNAME, NS, MX, TXT, SOA) | `dnsx` | `src/app/probe/dns.rs` | Active | None | `id`, `source: "dnsx"`, `checked_at` | CLI + TUI |
| HTTP & TLS inspection + tech fingerprinting | `httpx` | `src/app/probe/http.rs` | Active | None | `id`, `source: "httpx"`, `checked_at`, `tls`, `redirect_chain` | CLI + TUI |
| ASN lookup | `asnmap` (+ Team Cymru / WHOIS fallback) | `src/app/probe/asn.rs` | Active / Query | Optional `asnmap` key | `id`, `source`, `checked_at` | CLI + TUI |
| Domain registration | RDAP / WHOIS | `src/app/probe/rdap.rs` | Query | None | `DomainRegistrationRecord`, `source` | CLI + TUI |

**Limitations & failure modes:**
- External tools must be on `PATH`; missing binaries fail visibly (`EXIT_EXTERNAL_TOOL_ERROR`) or record failed stages in `ScanReport`.
- DNS/HTTP probes are subject to bounded timeouts and scope checks (`src/app/probe/scope.rs`).
- Active DNS cannot enumerate other domains hosted on an IP; that requires local correlation or passive indexes.

## B. Native Balebos Capabilities

| Capability | Location | Input / Output | CLI / TUI | Test Coverage |
|---|---|---|---|---|
| Domain normalisation & validation | `src/domain/normalize.rs`, `src/tui/model.rs` | Raw host → canonical lowercase domain / apex | Both | Unit tests |
| SQLite operational store | `src/storage/sqlite.rs` | Append-only `records`, `quarantine`, `archive_state` | Both | Unit tests |
| Sealed Parquet evidence archive | `src/storage/archive.rs` (`--features parquet-archive`) | Deterministic segments + SHA-256 manifests | CLI | Unit & crash-recovery tests |
| Snapshot assembly & scan coverage | `src/app/snapshot.rs`, `src/domain/coverage.rs` | `Records` → `SnapshotView` + `ScanReport` | Both | Unit tests |
| Asset drift detection | `src/drift.rs`, `src/app/drift.rs` | Baseline vs current → `DriftFinding` + `DriftEvidence` | Both | Unit tests |
| Hosting heuristics | `src/hosting/detector.rs` | DNS + HTTP + ASN → `HostingObservation` | Both | Unit tests |
| Relationship projection & queries | `src/domain/relationship.rs`, `src/app/relationships.rs` | `Records` → `ObservedRelationship`, `related`, `explain`, `history` | CLI + TUI (`AssetDetails`) | Unit tests |
| Controlled pivot discovery | `src/app/pivot.rs` | Seed + `Records` + `PivotConfig` → `PivotCandidate` | CLI | Unit tests |
| Discovery effectiveness metrics | `src/app/metrics.rs` | `Records` + `PivotReport` → `DiscoveryMetricsReport` | CLI | Benchmark fixture tests |
| Findings & risk assessment | `src/app/check.rs`, `src/app/risk.rs`, `src/app/finding.rs` | `Records` → `Finding` + `RiskScore` | Both | Unit tests |

## C. Data Enrichment Providers

| Provider | Location | Mode | Credentials | Limitations |
|---|---|---|---|---|
| Certificate Transparency (`crt.sh` JSON contract + bounded live opt-in) | `src/app/sources.rs` | Passive | None | Public CT logs may lag; wildcard SANs are patterns; issuance does not prove live deployment |
| Passive DNS (provider-neutral import contract + fixture adapter) | `src/app/sources.rs` | Passive | Optional per external provider | Provider `first_seen`/`last_seen` are external claims, never proof of uninterrupted existence |

## D. Planned Capabilities (Not Yet Implemented)

- Live paid Passive DNS provider connectors (SecurityTrails / PassiveTotal / Farsight) — blocked on API licensing/credentials; contract and fixture workflow implemented.
- First-party read-only cloud estate adapters (AWS Route 53, CloudFront, ACM, Terraform state) — deferred until core correlation benchmarks are established.
- Graphical relationship graph UI — explicitly deferred per ADR 0011.
