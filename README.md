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
- tracks HTTP redirect chains (internal vs external redirects)
- monitors TLS certificate expiration and issuer details
- stores state in a simple, queryable form (JSONL / Parquet / DuckDB), optimized for low cost and ad-hoc analysis
- tracks changes over time
- exposes system state via API / queries
- highlights drift and unknowns

## Documentation

- **Guides**
  - [Getting Started](documentation/guides/getting-started.md) - Installation and first steps
  - [Reconnaissance Pipeline](documentation/guides/recon-pipeline.md) - How assets are discovered and probed
  - [Usage Reference](documentation/guides/usage.md) - Detailed command documentation
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
- simple storage over complex infrastructure
- visibility over abstraction
- cost-aware by default

## Status

Alpha stage with working core functionality. Main features are implemented and tested.

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
```

### Data Storage

Balebos stores all data in append-only JSONL files located in the `data/` directory:
- `data/inventory.jsonl` - Main inventory file containing all assets

This approach ensures:
- Simple, human-readable storage
- Audit trail of all changes
- Easy backup and version control
- No database dependencies

### Core Concepts

1. **Inventory** - Centralized registry of all infrastructure assets
2. **Validation** - Structural checks ensure only valid domain names enter the system
3. **Normalization** - Assets are normalized for consistent storage and analysis
4. **Anomaly detection** - Identification of suspicious patterns and deviations
5. **Operational monitoring** - Regular reports highlighting changes requiring attention
6. **Non-destructive filtering** - Ignore patterns filter display without modifying stored data

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

Balebos includes an interactive Terminal User Interface for real-time inventory exploration:

```bash
balebos ui
```

```
┌─ Overview ────────────────────────────────────────────────────────────────────┐
│ Balebos Dashboard  | Assets: 645  Alive: 487  HTTP Errors: 23  DNS Issues: 8 │
└───────────────────────────────────────────────────────────────────────────────┘
┌─ Inventory (Space to toggle) ──────────┬─ Details ────────────────────────────┐
│ Asset               Status       Tech  │ Target: myapp.io                     │
│ [+] myapp.io        200 OK       Reac.│                                      │
│ [-] acme-corp.com   200 OK       Ngi..│ DNS Records (Last check: ...):        │
│   └─ api.acme-co    200 OK       -    │   A:     203.0.113.10               │
│   └─ www.acme-co    200 OK       Ngi..│   AAAA:  2001:db8::1                │
│ [+] legacy.net      EXT -> other  -   │                                      │
│     defunct.org      DnsNotFound   -   │ HTTP Info (Last check: ...):          │
│                                        │   Title:  My Application             │
│                                        │   Server: nginx                      │
│                                        │   Tech:   Nginx, HSTS               │
│                                        │   Final:  https://myapp.io           │
│                                        │                                      │
│                                        │   TLS Certificate:                   │
│                                        │     Issuer:  CN=DigiCert...          │
│                                        │     Expires: 2027-01-15 (296 days)   │
└────────────────────────────────────────┴──────────────────────────────────────┘
┌──────────────────────────────────────────────────────────────────────────────┐
│ q: Quit | /: Filter | x: Toggle Ext | a: Add | s: Scan | Space: Toggle    │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Key features:**
- **Hierarchical navigation** — Root domains with collapsible subdomain groups (`Space`/`Enter`)
- **Smart status column** — Distinguishes `200 OK`, external redirects (`EXT -> target.com`), HTTP errors, and DNS failures
- **Detail inspector** — Full DNS records, HTTP info (title, server, tech stack), TLS certificate details with expiry countdown, and redirect chain visualization
- **Live scanning** — Press `a` to add a new domain, `s` to rescan the selected asset — runs in the background with a real-time progress indicator in the header
- **Filtering** — Press `/` to search, `x` to hide external redirects
- **Redirect detection** — Correctly identifies external redirects (to a different root domain) while ignoring same-domain redirects (www, subfolders, sibling subdomains)

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

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/balebos.git
cd balebos

# Build the binary
cargo build --release

# Run Balebos
./target/release/balebos
```

### Dependencies

- Rust 2021 edition or newer
- ProjectDiscovery ecosystem tools (required for probes):
  - [subfinder](https://github.com/projectdiscovery/subfinder)
  - [dnsx](https://github.com/projectdiscovery/dnsx)
  - [httpx](https://github.com/projectdiscovery/httpx)
