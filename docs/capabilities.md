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
| Domain & FQDN normalisation | `src/domain/normalize.rs` | Raw host → `normalize_fqdn` (preserves `www.`) / `registrable_domain` (eTLD+1) | Both | Unit tests |
| SQLite operational store | `src/storage/sqlite.rs` | Append-only `records`, `quarantine`, `archive_state` | Both | Unit tests |
| Sealed Parquet evidence archive | `src/storage/archive.rs` (`--features parquet-archive`) | Deterministic segments + SHA-256 manifests | CLI | Unit & crash-recovery tests |
| Snapshot assembly & scan coverage | `src/app/snapshot.rs`, `src/domain/coverage.rs` | `Records` → `SnapshotView` + `ScanReport` | Both | Unit tests |
| Asset drift detection | `src/drift.rs`, `src/app/drift.rs` | Baseline vs current → `DriftFinding` + `DriftEvidence` | Both | Unit tests |
| Role-separated hosting attribution | `src/hosting/detector.rs` | DNS + HTTP + ASN + RDAP → `InfrastructureAttribution` (`Heuristic` / `Conflicting` / `Unknown` origin) | Both | Unit tests |
| Network service entity & fingerprint model | `src/domain/service.rs` | `ServiceObservation` (socket identity, `PortHypothesis` vs `IndexedBanner` vs `ObservedProtocol` vs `Mismatch`, SSH/TLS/interface fingerprints) | CLI (`services list\|show`) + TUI (`AssetDetails`) | Unit & benchmark tests |
| Application reference discovery | `src/app/references.rs` | Bounded static extraction from JS bundles, OpenAPI 3.x/Swagger 2.0, DNS SRV/TXT, public storage URLs, `urlscan.io` JSON, authorised public repo configs | CLI (`discover references`) + TUI (`AssetDetails`) | Benchmark fixture tests |
| Relationship projection & queries | `src/domain/relationship.rs`, `src/app/relationships.rs` | `Records` → `ObservedRelationship`, `related`, `explain`, `history` | CLI + TUI (`AssetDetails`) | Unit & E2E tests |
| Controlled pivot discovery | `src/app/pivot.rs` | Seed + `Records` + `PivotConfig` → `PivotCandidate` | CLI | Unit tests |
| Discovery effectiveness metrics | `src/app/metrics.rs`, `src/app/service_benchmarks.rs` | `Records` + `PivotReport` + `ServiceObservation` + `ApplicationReferenceObservation` → `DiscoveryMetricsReport` | CLI | 14-scenario + 22-scenario benchmarks + E2E test |
| Findings & risk assessment | `src/app/check.rs`, `src/app/risk.rs`, `src/app/finding.rs` | `Records` → `Finding` + `RiskScore` | Both | Unit tests |
| Data management | `src/cli/handlers/data.rs` | `--data-dir <dir>`, `balebos data info\|backup\|reset [--dry-run] [--backup]` | CLI | Unit / CLI |

## C. Data Enrichment Providers

| Provider | Location | Mode | Credentials | Limitations |
|---|---|---|---|---|
| Certificate Transparency (`crt.sh` JSON contract + bounded live opt-in) | `src/app/sources.rs` | Passive | None | Public CT logs may lag; wildcard SANs are patterns; issuance does not prove live deployment |
| Passive DNS (provider-neutral import contract + fixture adapter) | `src/app/sources.rs` | Passive | Optional per external provider | Provider `first_seen`/`last_seen` are external claims, never proof of uninterrupted existence |
| Censys Hosts v2 (offline JSON import/fixture parser) | `src/app/external_services.rs` | Passive | Optional (live paid API transport unbundled) | Indexed observations carry provider timestamps; never a live Balebos check |
| Shodan Host (offline JSON import/fixture parser) | `src/app/external_services.rs` | Passive | Optional (live paid API transport unbundled) | Banners may be stale or spoofed; port numbers never establish application protocol |
| `urlscan.io` (indexed result JSON parser) | `src/app/references.rs` | Passive (indexed) | Optional | Browser resource fetches do not prove backend dependencies; internal URLs are never auto-submitted |

## D. Epistemic Boundaries (What Balebos Can and Cannot Know)

1. **Discoverable from the public internet**: DNS records, TLS certificates, HTTP responses/headers/favicons, RDAP/WHOIS registrar metadata, indexed open ports/banners, public JavaScript/OpenAPI/SRV/TXT references, and public storage URLs.
2. **Only inferable (never proved automatically)**: Common ownership across shared IPs/ASNs/CDNs/certificates, origin hosting behind a CDN (`Unknown` by default), whether a shared SSH host key is the same host vs a cloned VM template, or whether a static JS/OpenAPI/repo reference is an active runtime dependency.
3. **Requires authorised active verification**: Whether an externally indexed port (`203.0.113.10:5432/tcp`) is currently reachable and actually speaks PostgreSQL rather than another protocol.
4. **Requires internal cloud or first-party estate access**: Origin servers behind CDNs, private RDS/ECS/Kubernetes workloads, IAM/account ownership, and internal bucket ACLs.
5. **Cannot be known reliably from external evidence alone**: Legal/organisational ownership from technical linkage alone, bucket-listing permissions from a public object URL, or uninterrupted service availability between external index timestamps.

## E. Planned Capabilities (Not Yet Implemented)

- Live credentialed API transports for paid Censys, Shodan, and Passive DNS plans (offline JSON/JSONL parsers and provider-neutral contracts are implemented).
- First-party read-only cloud estate adapters (AWS Route 53, CloudFront, ACM, Terraform state).
- Graphical relationship graph UI — explicitly deferred per ADR 0011.
