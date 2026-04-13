# Balebos

Balebos is a system control layer that keeps your infrastructure in a known, observable state.

It scans, normalizes and tracks assets (domains, services, repositories) and enforces basic operational invariants:
- everything is known
- everything is monitored
- nothing silently rots

Balebos is not just a dashboard.
It is not a SIEM.
It is a minimal control plane for reducing system entropy.

Balebos is designed so that system state does not depend on constant human attention.

## What it does

- discovers assets (domains, subdomains, repos)
- resolves DNS records (A, AAAA, CNAME) via dnsx
- performs deep HTTP probing (tech stack, title, web server) via httpx
- tracks domain registration and expiration details via rdap/whois
- tracks HTTP redirect chains (internal vs external redirects)
- monitors TLS certificate expiration and issuer details
- stores state in columnar format (Apache Parquet) with DataFusion query engine
- tracks changes over time
- exposes system state via API / queries
- highlights drift and unknowns
- persists findings with lifecycle tracking (acknowledge, resolve, auto-resolve)

## Quick Start

The fastest way to run Balebos with all its dependencies is using Docker:

```bash
# Start the interactive TUI
docker compose run --rm balebos ui

# Scan a domain
docker compose run --rm balebos scan example.com

# Run checks and persist findings
balebos finding run

# List open findings
balebos finding list

# Show risk report
balebos risk report
```

## Documentation

- **Guides**
  - [Getting Started](documentation/guides/getting-started.md) - Installation and first steps
  - [Reconnaissance Pipeline](documentation/guides/recon-pipeline.md) - How assets are discovered and probed
  - [Usage Reference](documentation/guides/usage.md) - Detailed command documentation
  - [Tag Management](documentation/guides/tag-management.md) - Asset tagging and organization
  - [Findings Management](documentation/guides/findings.md) - Persisted issues and lifecycle tracking
- **Technical Specs**
  - [Architecture Overview](documentation/architecture.md) - Design principles and layers
  - [Data Model](documentation/specs/data-model.md) - JSONL schema and record types

## Why

Most systems don’t fail loudly.
They decay:
- expired certificates
- forgotten services
- unmanaged domains
- silent data drift

Balebos exists to make that visible and controlled.

## Principles

- no magic
- deterministic pipelines
- simple storage over complex infrastructure (JSONL for interchange, Parquet for analytics)
- visibility over abstraction
- cost-aware by default

## Status

Beta stage with high-performance storage and container support.

## Usage

Balebos provides several commands for managing and monitoring your infrastructure:

```bash
# Scan a domain and add to inventory
balebos scan example.com

# Discover subdomains using subfinder
balebos probe subdomains example.com

# Resolve DNS records using dnsx
balebos probe dns example.com

# Check HTTP status of a domain
balebos probe http example.com

# List domains in inventory
balebos list
balebos list --unique

# Display domain hierarchy with nested subdomains
balebos tree

# Show domain live status with HTTP codes
balebos status

# Show domain statistics
balebos stats
balebos stats --by-domain

# Check for inventory issues and anomalies
balebos check

# Show changes (last 7 days)
balebos diff

# Generate consolidated status report
balebos report

# Launch interactive TUI dashboard
balebos ui

# Finding management
balebos finding run                    # Run checks and persist new findings
balebos finding list                   # List all findings
balebos finding list --status open     # List only open findings
balebos finding list --severity high   # List high severity findings
balebos finding show <id>              # Show finding details
balebos finding acknowledge <id>       # Acknowledge a finding
balebos finding resolve <id>           # Resolve a finding manually

# Asset lifecycle management
balebos asset lifecycle set <asset> <state> [--reason <text>]
                                       # Set asset lifecycle state
balebos asset lifecycle show <asset>           # Show asset lifecycle info
balebos asset lifecycle history <asset>        # Show lifecycle history
balebos asset lifecycle list                   # List assets by lifecycle state

# Risk scoring
balebos risk report                    # Show full risk report for all assets
balebos risk top [N]                   # Show top N highest risk assets (default: 10)
balebos risk show <asset>              # Show detailed risk score for an asset

# Tag management
balebos tag add example.com to-review
balebos tag add example.com priority-high
balebos tag list
balebos tag list example.com
balebos tag list-tags
balebos tag by-tag priority-high
balebos tag remove example.com to-review

# Organization management
balebos org brand add "Acme Corp"
balebos org brand list

balebos org company add "Acme DE" DE --brand 123456
balebos org company add "External Agency" PL  # no brand = external
balebos org company list

balebos org person add "Jan" "Kowalski" "jan@example.com"
balebos org person list

balebos org project add "Website" --company 789012
balebos org project list

balebos org assign asset api.example.com --project 345678
balebos org assign person 111222 --project 345678 --role owner
balebos org assign company 333444 --project 345678 --role technician
```

### Data Storage

Balebos uses a tiered storage approach:
- **Apache Parquet** - Columnar storage for fast analytical queries
- **DataFusion** - Query engine with SQL support
- **JSONL (`.jsonl`)** - Backup, interchange, and audit trail

Storage is automatically partitioned by record type and date for optimal performance. JSONL files remain available for backup and external processing.

### Core Concepts

1. **Inventory** - Centralized registry of all infrastructure assets
2. **Validation** - Structural checks ensure only valid domain names enter the system
3. **Normalization** - Assets are normalized for consistent storage and analysis
4. **Anomaly detection** - Identification of suspicious patterns and deviations
5. **Findings** - Persisted issues with severity, category, and lifecycle (open/acknowledged/resolved)
6. **Asset Lifecycle** - Formal states: Active, Under Review, Deprecated, Decommissioned, Abandoned
7. **Auto-resolve** - Findings automatically close when underlying issues are fixed
8. **Operational monitoring** - Regular reports highlighting changes requiring attention
9. **Non-destructive filtering** - Ignore patterns filter display without modifying stored data
10. **Organizational context** - Assets linked to projects, companies, and brands for ownership tracking

### Organizational Model

Balebos tracks ownership and responsibility through a hierarchical organizational model:

```
Brand (e.g., "Acme Corp")
  └── Company (e.g., "Acme Deutschland GmbH")
        └── Project (cost center / MPK)
              └── Asset (domain/subdomain)
```

- **Brands** group related companies under a single umbrella
- **Companies** are legal entities (internal or external contractors)
- **Projects** are cost centers within a company
- **Assets** are assigned to projects for ownership tracking
- **Role assignments** link people or companies to projects (Owner, Manager, Technician)

External contractors (companies without a brand) can be assigned to projects via role assignments but don't own projects directly.

### Domain Hierarchy Visualization

The `balebos tree` command displays your domains in a hierarchical tree structure, showing the relationship between root domains and their discovered subdomains:

```
Domain Hierarchy (15)

├── 🔵 example.com
│   ├── ◇ api
│   ├── ◇ blog
│   ├── ◇ dev
│   │   ├── ◇ api
│   │   │   └── ◇ auth
│   │   └── ◇ web
│   └── ◇ www
│
├── 🔵 acme.test
│   ├── ◇ staging
│   │   └── ◇ portal
│   ├── ◇ tracking
│   │   └── ◇ analytics
│   └── ◇ www
│       └── ◇ staging
│           └── ◇ portal
```

**Legend:**
- 🔵 = Root domain (added to inventory via `balebos scan` or batch import)
- ◇ = Subdomain (discovered via `balebos probe subdomains`)

This view helps identify:
- Which domains have complex subdomain structures
- Multi-level nested subdomains (e.g., `auth.api.dev.example.com`)
- Organizational patterns in your infrastructure
- Domains without discovered subdomains (leaves in the tree)

### Domain Live Status Report

The `balebos status` command provides a comprehensive health report of all assets, showing HTTP probe results grouped by status:

```
Domain Status Report
====================
Total assets: 1001

Status Summary:
  🟢 Alive (HTTP 200):         654 ( 65.3%)
  🟡 Client errors (4xx):      133 ( 13.3%)
  🔴 Server errors (5xx):       32 (  3.2%)
  ⚠️  Network errors:           182 ( 18.2%)
  ⚪ Unchecked:                  0 (  0.0%)

🟢 ALIVE (HTTP 200) - 654 assets
...

🔴 SERVER ERRORS (5xx) - 32 assets
...

🟡 CLIENT ERRORS (3xx/4xx) - 133 assets
...

⚠️  NETWORK ERRORS - 182 assets
...
```

**Status Categories:**
- **🟢 Alive** - Domains responding with HTTP 200 (working correctly)
- **🟡 Client Errors** - HTTP 3xx/4xx responses (redirects, authentication, not found)
- **🔴 Server Errors** - HTTP 5xx responses (backend failures)
- **⚠️ Network Errors** - DNS failures, timeouts, TLS errors, connection errors
- **⚪ Unchecked** - Assets not yet probed

This report helps identify:
- Overall portfolio health percentage
- Which domains have infrastructure issues
- DNS resolution problems
- Services with configuration issues (4xx errors)
- Backend failures requiring attention (5xx errors)
- Network connectivity problems

### Interactive TUI Dashboard

Balebos includes an interactive terminal dashboard built with [ratatui](https://github.com/ratatui/ratatui).

```bash
balebos ui
```

The TUI provides:
- Inventory browsing with hierarchical subdomain view
- Asset details: DNS records, HTTP info, TLS certificates, redirect chains, lifecycle state
- Asset lifecycle management (set state, view history)
- Organization management (brands, companies, persons)
- Project management with asset assignments
- Findings management with severity filtering
- Live scanning and filtering

#### Exporting Status Reports

The status report can be exported to multiple formats for integration with other tools and for sharing with team members:

**CSV Export** - For analysis in spreadsheet applications:
```bash
# Export to default file (domain-status-report.csv)
balebos status --export csv

# Export to custom file
balebos status --export csv /path/to/report.csv
```

**Markdown Export** - For documentation and version control:
```bash
# Export to default file (domain-status-report.md)
balebos status --export markdown

# Export to custom file
balebos status --export markdown /path/to/report.md
```

**HTML Export** - For viewing in web browsers with styled formatting:
```bash
# Export to default file (domain-status-report.html)
balebos status --export html

# Export to custom file
balebos status --export html /path/to/report.html
```

**Export Features:**
- All formats include timestamp of report generation
- CSV: Tabular format with all asset details (target, status, HTTP code, response time, errors)
- Markdown: Summary statistics table plus detailed sections for each status category
- HTML: Styled report with color-coded status indicators, responsive layout, and sortable tables
- Default output files use descriptive names in the current directory
- All exports preserve complete data for later analysis

### Integration

Balebos integrates with external tools to leverage proven solutions:
- `subfinder` - For subdomain enumeration
- `dnsx` - For fast DNS resolution
- `httpx` - For deep HTTP probing, fingerprinting, and redirect tracking
- `rdap` - For domain registration and expiration data

## Installation

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/balebos.git
cd balebos

# Build the binary
make build

# Run Balebos
make run-ui
```

### Development

A `Makefile` is provided for common tasks:
- `make dev-check` - Quick compilation check
- `make dev-test` - Run all tests
- `make dev-fix` - Auto-format and lint fix
- `make clean` - Clean build artifacts and local DuckDB

### Dependencies

- Rust 1.88+
- Apache Parquet (for columnar storage)
- Infrastructure discovery tools (required for probes):
  - [subfinder](https://github.com/projectdiscovery/subfinder)
  - [dnsx](https://github.com/projectdiscovery/dnsx)
  - [httpx](https://github.com/projectdiscovery/httpx)
  - [rdap](https://github.com/openrdap/rdap) (or a compatible RDAP client)
