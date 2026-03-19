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
- stores state in a simple, queryable form (JSONL / Parquet / DuckDB), optimized for low cost and ad-hoc analysis
- tracks changes over time
- exposes system state via API / queries
- highlights drift and unknowns

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

# Check HTTP status of a domain
balebos probe http example.com

# List domains in inventory
balebos list
balebos list --unique

# Show domain statistics
balebos stats
balebos stats --by-domain

# Check for inventory issues and anomalies
balebos check

# Show changes (last 7 days)
balebos diff

# Generate consolidated status report
balebos report
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
2. **Normalization** - Assets are normalized for consistent storage and analysis
3. **Anomaly detection** - Identification of suspicious patterns and deviations
4. **Operational monitoring** - Regular reports highlighting changes requiring attention
5. **Non-destructive filtering** - Ignore patterns filter display without modifying stored data

### Integration

Balebos integrates with external tools to leverage proven solutions:
- `subfinder` - For subdomain enumeration
- HTTP checks - For web service monitoring

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
- For subdomain scanning: [subfinder](https://github.com/projectdiscovery/subfinder)
