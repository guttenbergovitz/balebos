# Changelog

All notable changes to Balebos are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
