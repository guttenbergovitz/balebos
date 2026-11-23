# Changelog

All notable changes to Balebos are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Bounded subprocess execution for all probe adapters: uniform
  wall-clock timeouts, bounded captured output (64 MiB ceiling), and on
  Unix process-group termination so timed-out scanners cannot leak
  orphaned grandchildren; killed children are always reaped. Replaces
  the platform-dependent `gtimeout`/`timeout` shell-outs and every
  unbounded `.output()` call (subfinder, dnsx, httpx, rdap, whois, dig,
  asnmap). `nix` added (unix-only process-group control).
- Inspection scope guard: `balebos probe` and `balebos batch` refuse
  private, loopback and link-local targets (and localhost-style names)
  unless `--allow-internal` or `BALEBOS_ALLOW_INTERNAL=1`. Refusals are
  reported and counted as coverage skips, never silent. Documented
  limit: literals only — pre-resolution enforcement is a follow-up.

### Changed

- Default storage engine is now SQLite (`data/balebos.db`); the legacy
  Parquet engine and the Polars dependency are removed from the default
  build (ADRs 0012/0013/0017). Legacy data imports via `balebos migrate`
  (build with `--features parquet-archive`).

### Measured (Phase E, cold `cargo build --release`, isolated target dir,
same machine, rustc 1.100.0)

| Metric | Baseline (pre-Phase E) | Now | Change |
|---|---|---|---|
| Cold release wall time | 4m 01s | 44.1s | **5.5× faster** |
| Release binary | 84,028,768 B (~80 MiB) | 7,028,656 B (~6.7 MiB) | **12× smaller** |
| Unique packages | 424 | 135 | **3.1× fewer** |
| Cold target dir | 1.1 GiB | 272 MiB | 4× smaller |

### Added

- Drift classification engine (ADR 0009): the fixed taxonomy
  (newly discovered, not observed, verified disappearance, DNS/HTTP/TLS/
  redirect/attribution/fingerprint change, inspection failure, insufficient
  evidence) as a pure, deterministic classifier over per-asset fact sets
  with explicit coverage. Absence is interpreted strictly through
  completeness; unobserved aspects report insufficient evidence without
  blocking comparable aspects.
- Snapshot assembly and `balebos drift [--within <hours>]`: materialises
  drift views from recorded observations (latest evidence per target at an
  optional time boundary; completeness from the latest scan report) and
  prints classified drift with evidence. Absences in an incomplete window
  are reported as "not observed", never as verified change.

## [2.0.0-alpha.1] - 2026-11-23

Pre-release of the 2.0 realignment: product scope, engineering foundation,
storage evidence integrity, SQLite operational engine and the sealed
evidence archive. The default engine remains the legacy Parquet store
until the migration path has been exercised on real data; see
`docs/migration-v2.md` and ADRs 0012-0018.

### Added

- Engineering foundation for Balebos 2.0: `docs/` durable documentation
  (product, architecture, conventions, operations, migration-v2), ADR
  framework with ADRs 0001–0010, and repository-local agent skills
  (`balebos-feature-slice`, `architecture-review`, `security-audit`,
  `storage-review`, `release-check`).
- `Taskfile.yml` as the canonical developer entry point; GitHub Actions CI
  (format, clippy `-D warnings`, rustdoc, tests, release build);
  `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`.
- Storage evidence integrity: unknown record types are quarantined verbatim
  instead of being coerced to `Domain`; unreadable records are reported in
  `Records::skipped` with reasons; atomic batch writes (temp file + fsync +
  rename); uncommitted records survive flush failures for retry.
- Domain vocabulary for the 2.0 model: `ObservationOutcome` (success,
  failed, skipped, unsupported, incomplete, unverified, confirmed absent —
  ADR 0005) and validated identity newtypes `AssetId`, `ScanId`,
  `ObservationId` (ADR 0004), serde-stable for persistence.
- Scan coverage: every `batch` run persists an identifiable `ScanReport`
  (scan id, timestamps, planned targets, per-target outcome — observed,
  failed, skipped, not attempted) so partial and failed scans stay
  distinguishable from verified change (ADR 0005).
- SQLite operational storage behind `BALEBOS_STORAGE=sqlite` (ADR 0012):
  append-only records, transactional all-or-nothing batches, per-write
  commits, quarantine table with reader-visible reporting.
- `balebos migrate [dir]`: verified, idempotent migration of legacy
  Parquet data into SQLite — per-row classification, verbatim quarantine
  of unknown/malformed rows, count verification, rename-not-delete
  rollback (ADR 0018).
- `balebos archive build|verify|restore` (feature `parquet-archive`):
  sealed evidence segments — deterministic rowid-prefix selection,
  immutable Parquet segments with typed columns, SHA-256 manifests with
  chain field, crash-safe publication, hash/count/chain verification and
  idempotent restore (ADRs 0013/0014). Default build unchanged.

### Changed

- `AGENTS.md` rewritten as the authoritative agent guide (product identity,
  non-goals, invariants, boundaries).
- CLI handler entry points are `pub` (lib + thin bin convention); strict
  Clippy adopted across the codebase.
- `documentation/` marked historical; `docs/` is the source of truth.

### Removed

- `Makefile`, `Dockerfile.dev` (docker workflows moved to Taskfile),
  `GEMINI.md`, stale merged feature branches, `evacuate/` migration
  artefacts, accidental `Users/` directory tree.

### Fixed

- Build on current stable Rust (ethnum 1.5.2 → 1.5.3).

## [1.5.0] - 2026-04-14

### Changed

- **Storage Migration to Polars/Parquet** - Replaced DuckDB with Polars-based Parquet backend
  - Per-record-type Parquet files for optimal performance (`domain.parquet`, `finding.parquet`, etc.)
  - Polars-based columnar processing for fast analytical queries
  - In-memory buffering with configurable batch size (default: 100 records)
  - Automatic schema inference from JSON objects
  - Thread-safe write operations via `Arc<Mutex<>>`
  - No server dependency—entirely file-based storage
  - JSONL remains available as interchange format

### Removed

- DuckDB dependency and adapter implementation
- DataFusion query engine

### Documentation

- Complete documentation reorganisation and update
- New guides: Findings Management, Asset Lifecycle, Risk Scoring, Organisation Model
- Updated Architecture documentation for Parquet storage
- New `documentation/README.md` navigation index
- All broken links fixed and references updated

## [1.4.0] - 2026-04-12

### Added

- **Persisted Findings System** - Replaced ephemeral issues with persisted finding records
  - Findings are stored as append-only records with UUID, severity, category, and target
  - Five severity levels: Critical, High, Medium, Low, Info
  - Six categories: Security, Compliance, Financial, Lifecycle, Operational, Governance
  - Lifecycle tracking via FindingEvent records (acknowledge, resolve)
  - Deduplication prevents duplicate findings for same check + target

- **Finding CLI Commands**
  - `balebos finding run` - Run checks and persist new findings with auto-resolve
  - `balebos finding list` - List findings with status/severity filters
  - `balebos finding list --status open|acknowledged|resolved` - Filter by status
  - `balebos finding list --severity critical|high|medium|low|info` - Filter by severity
  - `balebos finding show <id>` - Display finding details
  - `balebos finding acknowledge <id>` - Acknowledge a finding
  - `balebos finding resolve <id>` - Manually resolve a finding

- **Auto-Resolve for Findings** - Findings automatically close when conditions are fixed
  - Stale findings resolve when domain is seen again
  - HTTP error findings resolve when checks succeed
  - Low confidence findings resolve with multiple observations
  - TLS/connection findings resolve when connections succeed

- **Findings Tab in TUI** - New dedicated tab for issue management
  - Visual status indicators: ○ Open, ✓ Acknowledged, ● Resolved
  - Severity-based color coding
  - Filter by severity and status
  - Acknowledge/resolve actions from TUI
  - Background scan execution

- **Storage Cleanup** - `cleanup_old_records()` removes aged findings
  - Configurable retention period (default: 30 days for resolved findings)
  - Automatic backup before cleanup
  - Safe atomic file replacement

### Changed

- **Check Command** - Now persists findings automatically
  - Returns appropriate exit codes based on finding severity
  - Creates new findings with deduplication
  - Triggers auto-resolve for fixed issues

- **Report Command** - Enhanced to include findings summary
  - Counts findings by severity
  - Shows open vs resolved statistics

## [1.3.0] - 2026-03-23

### Added

- **RDAP/WHOIS Domain Registration Probing** - Query domain registration data via RDAP protocol with WHOIS fallback
  - Captures registrar, registration/expiration dates, and EPP status codes
  - Records registrant, admin, and tech contact information
  - New `DomainRegistrationRecord` storage format (`item_type: "org_domain_registration"`)
  - Integrated into batch processing for bulk domain intelligence
- **External Contractor Assignments** - Assign external companies to projects with contractor roles
  - Supports roles: Development, DevOps, Design, Consultancy, Security
  - New `ExternalContractorAssignment` record type

### Changed

- **TUI Asset Details** - Now displays domain registration information when available

## [1.1.0] - 2026-03-21

### Added

- **TLS Certificate Probing** - Integrated `httpx`'s `-tls-probe` to capture certificate details
  - Records Issuer DN, Subject DN, Expiration Dates, and Fingerprints
  - Stores certificate state in `TlsObservation` events
- **Certificate Monitoring & Alerts** - `balebos check` now monitors TLS health
  - Generates critical alerts for expired certificates
  - Warns about certificates expiring within 14 and 30 days
- **TUI Certificate Inspector**
  - New "TLS Certificate" section in the details panel
  - Visual urgency indicators: Red for <14 days, Yellow for <30 days, Green for healthy certs

### Changed

- **Version 1.1 Milestone** - Enhanced infrastructure observability with proactive security monitoring

## [1.0.0] - 2026-03-21

### Added

- **Live TUI Filtering** - Press `/` or `f` to filter domains
  - Instant searching across all root domains in the inventory
  - Filter state persists until cleared, with a dedicated filter status bar
  - Keyboard-friendly workflow: `Esc` or `Enter` to return to navigation
- **UX Enhancements**
  - Added dedicated Filter Bar in the dashboard
  - Updated footer with new keybindings for discoverability
  - Improved details panel layout for better readability of rich DNS records

### Changed

- **Version 1.0 Milestone** - Balebos is now a stable, high-performance orchestrator for infrastructure visibility

## [0.9.0] - 2026-03-21

### Added

- **Interactive TUI Dashboard** - Launch with `balebos ui`
  - Real-time inventory browser using Ratatui
  - Hierarchical domain grouping with Expand/Collapse support
  - Detailed asset inspector showing full tech stack and redirect history
  - Background scanning: Add and probe domains without leaving the UI
- **Rich DNS Data Collection**
  - Now captures and displays: `A`, `AAAA`, `CNAME`, `MX`, `NS`, `TXT`, and `SOA` records
  - Timestamps for last successful DNS and HTTP checks shown in the UI inspector
- **Concurrency & UI Polish**
  - Integrated asynchronous background thread for non-blocking recon tasks
  - Live status indicator `[SCANNING...]` in the dashboard header

### Changed

- **Storage Model** - Expanded `DnsObservation` to support rich record types
- **Internal API** - `JsonlStorage` now implements `Clone` for easier multi-threaded access

## [0.7.0] - 2026-03-21

### Added

- **High-Performance Streaming Architecture** - Completely refactored `batch` processing to use streaming pipes
  - Integrated `dnsx` and `httpx` using a single-process, bulk-input approach
  - Implemented background threading for non-blocking stdin feeding
  - Real-time JSON output parsing for immediate inventory updates
  - Massive performance boost for large-scale domain discovery and probing

### Changed

- **Batch Workflow** - DNS and HTTP steps now process the entire asset list in one pass instead of looping per-domain
- **Documentation** - Updated architecture and pipeline guides to reflect the new concurrency model

## [0.6.0] - 2026-03-21

### Added

- **Advanced HTTP Probing** - Integrated `httpx` (ProjectDiscovery) for robust web analysis
  - Collects technology stacks (`tech`), page titles (`title`), and web server fingerprints (`webserver`)
  - Maps `httpx` JSON output into enriched `HttpObservation` models
  - Protects against incorrectly aliased Python `httpx` package with guided error messages
- **CLI Outputs** - Detailed web server, title, and technology stacks shown natively during `probe http`

### Changed

- **Removed `reqwest` & `url` Dependencies** - Delegated HTTP probing entirely to `httpx` binary for better performance and alignment with Balebos architecture
- **HTTP Model Expansion** - The `HttpObservation` schema now safely incorporates `tech`, `webserver`, `title`, `cdn_name`, and `favicon_hash` fields (with backward compatibility)

## [0.5.0] - 2026-03-21

### Added

- **DNS Record Probing** - Integration with `dnsx` for resolving domains and capturing records
  - Captures A, AAAA, and CNAME records in `DnsObservation` events
  - New `balebos probe dns <domain>` command
  - Integrated into `batch` process as Step 3/4 (Discovery -> DNS -> HTTP)

### Changed

- **Batch Workflow** - Expanded to 4 steps to include DNS resolution before HTTP checks
- **Storage Reader** - Added support for parsing `dns_record` entries from JSONL logs

## [0.4.0] - 2026-03-21

### Added

- **HTTP Redirect Chain Tracking** - Manual following of HTTP redirects (up to 10 hops)
  - Captures full redirect history: status codes (301, 302, etc.) and intermediate URLs
  - Resolves relative redirect locations to absolute URLs
  - Records the final destination URL in observations
  - Detects and reports "Too many redirects" as an anomaly

- **Enhanced Status Reports** - Redirect information included in all report formats
  - CLI: `balebos status` shows `(via 301)` for redirected alive assets
  - CSV/Markdown/HTML: New columns and detailed redirect statistics included in exports

### Changed

- **HTTP Probe Engine** - Switched from automatic to manual redirect handling for better visibility
- **Dependencies** - Added `url` crate for robust URL parsing and relative path resolution

## [0.3.0] - 2026-03-21

### Added

- **Domain Validation** - New structural check for domain names before scanning
  - Ensures presence of TLD (missing dot)
  - Validates characters (alphanumeric and hyphens)
  - Prevents adding invalid domains to the inventory

### Changed

- **CLI Architecture** - Replaced cascading CLI dispatch with a clean `Command` enum and `match` pattern
- **Internal Storage Reader** - Extracted shared JSONL reader logic, eliminating multiple duplicate implementations of `RecordHeader` handling
- **Code Quality** - Removed dead code, no-op branches, and resolved all clippy warnings for idiomatic Rust

## [0.2.0] - 2026-03-20

### Added

- **Domain Status Report** - New `balebos status` command showing domain health with HTTP status breakdown
  - Categorizes assets by status: alive (200), client errors (4xx), server errors (5xx), network errors
  - Shows statistics (count and percentage) for each category
  - Displays sample listings with response times and error information
  
- **Export Formats** - Status reports can be exported to multiple formats
  - CSV export for spreadsheet analysis
  - Markdown export for documentation and version control
  - HTML export with styled formatting for web viewing
  - Support for custom output filenames
  
- **Domain Hierarchy Visualization** - New `balebos tree` command showing nested domain structure
  - Displays root domains and discovered subdomains in hierarchical format
  - Helps identify complex subdomain structures and organizational patterns
  
- **Batch Processing** - New `balebos batch` command for full reconnaissance
  - Combines scanning, subdomain discovery, and HTTP probing in single command
  
- **Network Error Capture** - HTTP probe now captures network errors as observations
  - Records DNS failures, timeouts, TLS errors, connection errors
  - Provides detailed error information for troubleshooting
  
- **Exit Code Mapping** - HTTP probe results now mapped to appropriate exit codes
  - Enables integration with shell scripts and CI/CD pipelines
  
- **Ignore Mechanism** - Non-destructive filtering via `balebos.ignore` file
  - Filters display output without modifying stored data
  - Supports exact matching and glob patterns
  
- **HTTP Probe Anomaly Detection** - Check command now detects HTTP probe errors as anomalies

### Changed

- Refactored inventory system to support heterogeneous JSONL records
  - `RecordHeader` pattern for handling multiple record types
  - List, stats, diff, and check commands updated to work with multiple record types
  
### Fixed

- Suppressed unused code warnings for fields and methods reserved for future functionality

## [0.1.0] - 2026-01-15

### Added

- Initial release with core functionality
- Asset discovery (domains, subdomains, repositories)
- JSONL-based storage system
- Basic listing and inventory management
- Statistics generation
- Change tracking (diff command)
- Anomaly detection (check command)
