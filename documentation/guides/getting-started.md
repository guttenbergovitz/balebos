# Getting Started with Balebos

This guide will help you get up and running with Balebos quickly.

## Installation

### Option A: Using Docker (Easiest)

Balebos is containerized to handle all dependencies (DuckDB, ProjectDiscovery tools) automatically.

```bash
# Start the TUI
docker compose run --rm balebos ui

# Scan a domain
docker compose run --rm balebos scan example.com
```

### Option B: Building from Source

Requires Rust 1.88+ and DuckDB installed on your system.

```bash
# Clone the repository
git clone https://github.com/yourusername/balebos.git
cd balebos

# Build the binary (automatically handles DuckDB linking if using Homebrew)
make build

# Run the TUI
make run-ui
```

### Local Prerequisites

If building from source, you need:
- **DuckDB**: `brew install duckdb` (macOS)
- **ProjectDiscovery tools**: [subfinder](https://github.com/projectdiscovery/subfinder), [httpx](https://github.com/projectdiscovery/httpx), [dnsx](https://github.com/projectdiscovery/dnsx)
- **whois**: `apt install whois` or `brew install whois`

## Quick Start

### 1. Add Your First Domain

```bash
balebos scan example.com
```

This adds `example.com` to your inventory.

### 2. Discover Subdomains (requires subfinder)

```bash
balebos probe subdomains example.com
```

This finds all known subdomains for the domain.

### 3. Check HTTP Status

```bash
balebos probe http example.com
```

This performs HTTP checks on the domain and captures the response status.

### 4. View Your Infrastructure

```bash
# See all domains
balebos list

# View in hierarchical format
balebos tree

# Check health status
balebos status

# Or launch the interactive dashboard
balebos ui
```

### 5. Interactive Dashboard (TUI)

For hands-on exploration, launch the TUI:

```bash
balebos ui
```

From the dashboard you can:
- Browse all domains and subdomains in a collapsible tree
- See HTTP status, tech stack, DNS records, and TLS certificate details at a glance
- Press `a` to add and scan a new domain (runs in the background with live progress)
- Press `s` to rescan the selected asset
- Press `/` to filter, `x` to hide external redirects
- Press `q` to quit

## Batch Processing

For a faster initial reconnaissance, use the batch command:

```bash
balebos batch example.com
```

This combines scanning, subdomain discovery, and HTTP checking in a single command.

## Monitoring Your Infrastructure

### View Status Summary

```bash
balebos status
```

Shows which domains are alive, have errors, or network issues.

### Generate Reports

```bash
balebos report
```

Comprehensive report including statistics, recent changes, and issues.

### Check for Problems

```bash
balebos check
```

Analyzes your inventory for:
- Stale domains (not seen recently)
- Low confidence items
- HTTP errors
- Missing checks

### Track Changes

```bash
balebos diff
```

Shows what's changed in the last 7 days.

## Exporting Data

### Export Status Report to HTML

```bash
balebos status --export html
```

Creates `domain-status-report.html` with styled visualization.

### Export to CSV

```bash
balebos status --export csv /path/to/report.csv
```

For analysis in spreadsheet applications.

### Export to Markdown

```bash
balebos status --export markdown /path/to/report.md
```

For documentation and version control.

## Filtering Noisy Data

If you have test domains or staging environments triggering warnings:

Create a `balebos.ignore` file:

```
# Test domains
*.test.*
*-temp.*

# Staging environments
staging-*
*.staging.*

# Development
dev-*
```

These patterns will be excluded from reports without modifying stored data.

## Integration Examples

### Automated Daily Status Report

```bash
# Add to crontab
0 9 * * * /path/to/balebos status --export html > /var/www/html/status.html
```

### Slack Notifications

```bash
balebos check | grep -q "issues" && curl -X POST \
  -H 'Content-type: application/json' \
  --data '{"text":"Infrastructure issues detected"}' \
  $SLACK_WEBHOOK_URL
```

### CI/CD Integration

```bash
# Fail the build if critical issues are found
balebos check
```

## Common Workflows

### Initial Infrastructure Scan

```bash
# Scan multiple domains
for domain in domain1.com domain2.com domain3.com; do
  balebos batch $domain
done

# View the hierarchy
balebos tree

# Generate initial report
balebos report
```

### Continuous Monitoring

```bash
#!/bin/bash
# Daily monitoring script

# Update HTTP status
for domain in $(balebos list --unique); do
  balebos probe http $domain
done

# Generate and export report
balebos status --export html /reports/status.html
balebos report > /reports/daily-report.txt

# Check for issues
balebos check > /reports/issues.txt
```

## Next Steps

- Read the [Usage Guide](guides/usage.md) for detailed command reference
- Check the [Architecture Documentation](architecture.md) for design details
- Review the [CHANGELOG](../CHANGELOG.md) to see what's new in v0.2.0

## Troubleshooting

### Invalid domain format

If you receive an error like `invalid domain 'example': 'example' has no TLD (missing dot)`, ensure:
- The domain includes a valid TLD (e.g., `.com`, `.net`, `.io`)
- The domain name contains only alphanumeric characters and hyphens
- You are not scanning a local hostname (like `localhost` or `server1`)

### No domains found after scanning

Check that the domain was added correctly:
```bash
balebos list
```

### HTTP checks showing network errors

This is normal for:
- Unreachable hosts (check DNS resolution)
- Services behind firewalls
- Invalid certificates (with strict TLS validation)

Use `balebos status` to see details about each error.

### High memory usage with large inventories

Balebos now supports DuckDB, which solves most performance issues. Ensure you are using the DuckDB backend:
```bash
export BALEBOS_STORAGE=duckdb
```
For very large inventories (>1M records), DuckDB's analytical engine will significantly outperform sequential JSONL processing.

## Getting Help

- Check command help: `balebos --help` or `balebos <command> --help`
- Review documentation files in the `documentation/` directory
- Check the README.md for overview and features
