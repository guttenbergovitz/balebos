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
- stores state locally in SQLite — transactional, append-only, no server or database required
- archives historical evidence into sealed Parquet segments with SHA-256 manifests (optional `parquet-archive` build)
- records scan coverage: every batch run persists an identifiable report of what was planned and what happened to each target (observed, failed, skipped, not attempted)
- tracks changes over time and highlights drift and unknowns
- discovers and correlates infrastructure relationships (`related`, `explain`, `history`, `pivot`) across domains, FQDNs, IPs, ASNs, TLS certificates, network services and application references
- ingests optional external service intelligence (Censys, Shodan, `urlscan.io`, Certificate Transparency, Passive DNS) with provider timestamps and explicit uncertainty
- extracts bounded application references from JavaScript assets, OpenAPI/Swagger specs, DNS SRV/TXT records, cloud storage URLs and public repository configs
- provides a 5-tab interactive TUI Investigation Workbench (`Overview`, `Runs`, `Explore`, `Changes`, `Findings`)
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
  - [Capabilities](docs/capabilities.md) - Code-verified capability matrix and epistemic boundaries
  - [Conventions](docs/conventions.md) - Rust standards, testing, Git discipline
  - [Operations](docs/operations.md) - Quality gate, tasks, release procedure
  - [Migration to 2.0](docs/migration-v2.md) - Data, CLI and feature disposition
  - [Architecture Decision Records](docs/adr/) - Recorded decisions (0001–0031)
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

**Infrastructure relationships, services & references:**
```bash
balebos discover run example.com --save             # Bounded CT, DNS SRV/TXT, JS & OpenAPI discovery
balebos discover services 203.0.113.10 --provider censys --input host.json --save
balebos discover references app.example.com --kind js --input bundle.js --save
balebos services list                               # List persisted network services & fingerprints
balebos services show 203.0.113.10:5432/tcp         # Inspect protocol evidence & uncertainty
balebos related example.com                         # Direct & shared-infrastructure relationships
balebos explain a.example.com b.example.com         # Explain supporting evidence between two entities
balebos pivot example.com --scope example.com       # Controlled local pivot discovery
balebos candidates list                             # Review passive discovery candidates
balebos candidates explain 203.0.113.10:5432/tcp    # Inspect candidate provenance & scope
balebos discovery report --json                     # Discovery yield, novelty & precision metrics
```

**Storage, backups and evidence:**
```bash
balebos --data-dir ./custom-data data info          # Show SQLite path, size and record counts
balebos data backup                                 # WAL-safe verified SQLite backup (VACUUM INTO)
balebos data reset --dry-run                        # Preview operational DB reset (archive preserved)
balebos migrate                                     # Import legacy Parquet data into SQLite (verified, reversible)
balebos archive build                               # Seal unarchived observations into a Parquet segment
balebos archive verify                              # Validate segment hashes, counts and chain
```

> Archive commands require a build with `--features parquet-archive`.

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

Balebos keeps operational state in a local SQLite database (`data/balebos.db`):

- **Transactional and append-only** - observations are inserted and never modified; every write commits before the command returns
- **Quarantine** - records with unknown types or malformed content are preserved verbatim and reported on read, never silently dropped or reclassified
- **Scan coverage** - every batch run records what was planned and what happened to each target

Historical evidence is archived explicitly (build with `--features parquet-archive`):

- `balebos archive build` seals unarchived observations into immutable Parquet segments — never rewritten
- Each segment carries a versioned SHA-256 manifest: record counts, schema version, boundary, hash chain
- `balebos archive verify` re-validates segments (hash, counts, chain continuity); `balebos archive restore <id>` re-imports them

Legacy 1.x Parquet data imports losslessly: `balebos migrate [dir]` classifies every row, quarantines the unrecognised, verifies counts, and renames (never deletes) the old directory for rollback.

### Storage Considerations

- **Single-process ownership** - one Balebos process owns the database; no inter-process locking
- **Retention** - observations stay in SQLite until sealed into a verified archive segment; no automated removal
- **Local-first** - no server, broker or hosted backend is ever required

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
- SQLite (bundled via `rusqlite` — no system dependency)
- Infrastructure discovery tools (required for probes):
  - [subfinder](https://github.com/projectdiscovery/subfinder)
  - [dnsx](https://github.com/projectdiscovery/dnsx)
  - [httpx](https://github.com/projectdiscovery/httpx)
  - [rdap](https://github.com/openrdap/rdap) (or a compatible RDAP client)

The optional `parquet-archive` feature adds Parquet segment support for
long-term evidence archival (see `balebos archive --help`).
