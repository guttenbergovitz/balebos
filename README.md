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

- discovers assets (domains, subdomains) via subfinder
- resolves DNS records (A, AAAA, CNAME) via dnsx
- performs deep HTTP probing (tech stack, title, web server) via httpx
- tracks domain registration and expiration details via rdap/whois
- tracks HTTP redirect chains (internal vs external redirects)
- monitors TLS certificate expiration and issuer details
- stores state locally in Apache Parquet files with Polars query engine — no server or database required
- records scan coverage: every batch run persists an identifiable report of what was planned and what happened to each target (observed, failed, skipped, not attempted)
- tracks changes over time and highlights drift and unknowns
- persists findings with lifecycle tracking (acknowledge, resolve, auto-resolve)
- exports an interactive static web dashboard for stakeholders

## Quick Start

### Using Docker (Recommended)

The fastest way to run Balebos with all its dependencies is using Docker:

```bash
# Start the interactive TUI
docker compose run --rm balebos ui

# Scan a domain
docker compose run --rm balebos scan example.com

# Run checks and persist findings
docker compose run --rm balebos finding run

# List open findings
docker compose run --rm balebos finding list

# Show risk report
docker compose run --rm balebos risk report
```

### Using Local Binary

If you've built Balebos from source with all dependencies installed:

```bash
# Start the interactive TUI
balebos ui

# Scan a domain
balebos scan example.com

# Run checks and persist findings
balebos finding run

# List open findings
balebos finding list

# Show risk report
balebos risk report
```

## Documentation

- **Current (Balebos 2.0)**
  - [Product definition](docs/product.md) - Identity, scope and explicit non-goals
  - [Architecture](docs/architecture.md) - Module boundaries and dependency direction
  - [Conventions](docs/conventions.md) - Rust standards, testing, Git discipline
  - [Operations](docs/operations.md) - Quality gate, tasks, release procedure
  - [Migration to 2.0](docs/migration-v2.md) - Data, CLI and feature disposition
  - [Architecture Decision Records](docs/adr/) - Recorded decisions (0001–0011)
- **Guides (1.x era, historical)**
  - [Getting Started](documentation/guides/getting-started.md) - Installation and first steps
  - [Reconnaissance Pipeline](documentation/guides/recon-pipeline.md) - How assets are discovered and probed
  - [Usage Reference](documentation/guides/usage.md) - Detailed command documentation
  - [Tag Management](documentation/guides/tag-management.md) - Asset tagging and organization
  - [Findings Management](documentation/guides/findings.md) - Persisted issues and lifecycle tracking

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

Beta. The Balebos 2.0 realignment is in progress (product scope, engineering
foundation and storage evidence integrity landed; see `docs/adr/` and
`CHANGELOG.md`). CI runs the full quality gate on every push to `develop`
and `main`.

## Usage

Balebos is built for hands-on infrastructure management. Here are the key workflows:

**Quick start:**
```bash
balebos scan example.com                    # Add domain to inventory
balebos probe subdomains example.com        # Discover subdomains
balebos probe http example.com              # Check HTTP status
balebos ui                                  # Launch interactive dashboard
```

**Risk and findings:**
```bash
balebos finding run                         # Run checks and persist findings
balebos risk report                         # Show prioritised risk assessment
balebos finding list --severity high        # List critical issues
```

**Infrastructure visibility:**
```bash
balebos list                                # Show all assets
balebos tree                                # Hierarchical view
balebos status                              # Health report by status code
```

**Batch scanning (with coverage):**
```bash
balebos batch domains.txt                   # Full recon; persists a scan report
                                            # (planned vs observed/failed/skipped per target)
```

**Ownership and context:**
```bash
balebos tag add api.example.com production  # Tag assets for local grouping
balebos asset lifecycle set api.example.com active  # Mark lifecycle state
```

> The `org` command family (Brand → Company → Project governance) still works
> but is slated for retirement in 2.0 — see ADR 0008. Prefer tags, lifecycle
> states and lightweight owner references for new workflows.

For comprehensive command reference, advanced filters, batch processing, and export options, see the [Usage Guide](documentation/guides/usage.md).

### Data Storage

Balebos uses a high-performance storage approach:
- **Apache Parquet** - Columnar storage partitioned by record type (domains, findings, DNS records, etc.)
- **Polars** - In-memory DataFrame engine for efficient data processing and queries
- **Memory Buffering** - Records are buffered (default: 100 items) before being written to Parquet for optimal I/O

Records are automatically partitioned by item type (domain, subdomain, http_check, dns_record, finding, etc.) with each type stored in its own Parquet file. This structure enables fast data retrieval and efficient batch operations while maintaining the entire history of all observations.

### Storage Considerations

- **In-Memory Loading** - All queries load data into memory. For datasets >1M records, consider filtering by time range or asset subset
- **All Fields are Text** - Parquet schema uses UTF-8 strings for all fields (efficient for infrastructure metadata)
- **Append-Only** - New observations are merged (vstack) with existing records, preserving complete history
- **No Live Indexes** - Lookups scan the entire Parquet file; acceptable for typical infrastructure inventories (10K-100K assets)

For very large-scale deployments (100K+ assets with daily updates), consider integrating a time-series database like ClickHouse or PostgreSQL for real-time queries while keeping Balebos for historical analysis and reporting.

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

### Web Dashboard

Balebos can export an interactive static web dashboard for sharing infrastructure status with stakeholders who don't use the terminal.

```bash
# Export data and build the site
./build-web.sh

# Or manually:
cargo run --release -- web export --output-dir web/static/data
cd web && zola build
```

The dashboard is built with [Zola](https://www.getzola.org/) (static site generator) and [Alpine.js](https://alpinejs.dev/) (lightweight reactivity). It features:

- **Solarized Dark** theme with **JetBrains Mono** for technical data
- **Foldable TLD groups** — root domains always visible, subdomains collapsed by default
- **Side panel detail view** — click any asset to see DNS, HTTP, TLS, hosting, and technologies
- **Technology pills** — all detected tech shown as inline tags (no truncation)
- **Client-side pagination** — 15 domains per page
- **Search and filter** — by name, lifecycle state, and risk level
- **Sortable columns** — click headers to sort
- **Three tabs**: Inventory, Findings, Risk Distribution

**Deploying:**
The `web/public/` directory contains a fully static site. Serve it with any web server:

```bash
# Preview locally
cd web && zola serve

# Deploy to nginx
cp -r web/public/* /var/www/html/

# Deploy to GitHub Pages
# (push web/public to gh-pages branch)
```

**Requirements:**
- [Zola](https://www.getzola.org/documentation/getting-started/installation/) must be installed
- The `web/static/data/` directory is gitignored — it is recreated on each export

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

### Using Docker (Recommended for Development)

All dependencies are bundled in the container:

```bash
# Build the Docker image
task docker:build

# Run the TUI
task docker:ui

# Scan a domain
task docker:scan -- example.com

# Or use docker-compose directly
docker compose run --rm balebos ui
docker compose run --rm balebos scan example.com
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/guttenbergovitz/balebos.git
cd balebos

# Build the binary
task build:release

# Run Balebos
cargo run -- ui
```

### Development

`Taskfile.yml` is the canonical entry point:
- `task check` - Full quality gate (format, clippy, rustdoc, tests)
- `task fmt` / `task lint` / `task test` / `task doc` - Individual gate steps
- `task smoke` - Offline smoke check
- `task docker:build` etc. - Container workflows

See `docs/operations.md` for the full list.

### Dependencies

- Rust (toolchain pinned in `rust-toolchain.toml`; installed automatically by rustup)
- Apache Parquet (for columnar storage)
- Infrastructure discovery tools (required for probes):
  - [subfinder](https://github.com/projectdiscovery/subfinder)
  - [dnsx](https://github.com/projectdiscovery/dnsx)
  - [httpx](https://github.com/projectdiscovery/httpx)
  - [rdap](https://github.com/openrdap/rdap) (or a compatible RDAP client)
