# Changelog

All notable changes to Balebos are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
