# Balebos - Usage Guide

This document provides detailed information on how to use Balebos to manage and monitor your infrastructure.

## Core Philosophy

Balebos is built on the principle of minimal, effective infrastructure control:
- Append-only data storage maintains complete history
- Data interpretation happens at query time, not at storage time
- Focus on anomaly detection rather than just data collection
- Synthesis of information into actionable reports

## Command Reference

### Scanning and Data Collection

#### Adding Domains

```bash
balebos scan example.com
```

This command:
- Normalizes the domain name (lowercase, removes www, etc.)
- Creates a domain entity with UUID and timestamp
- Stores it in the inventory

#### Discovering Subdomains

```bash
balebos probe subdomains example.com
```

This command:
- Uses subfinder to enumerate subdomains
- Adds each subdomain to the inventory
- Marks the source as "subfinder"

#### Checking HTTP Status

```bash
balebos probe http example.com
```

This command:
- Attempts HTTPS connection with fallback to HTTP
- Records HTTP status code and response time
- Captures network errors (DNS failures, timeouts, TLS errors, etc.) as observations
- Adds observation to inventory with timestamp
- Returns exit code based on HTTP status (for automation)

#### Full Reconnaissance Batch

```bash
balebos batch example.com
```

This command combines scanning, subdomain discovery, and HTTP checking in a single operation:
- Scans the domain and adds it to inventory
- Discovers subdomains using subfinder
- Performs HTTP checks on all discovered domains
- Adds all observations to inventory with timestamps

### Querying Data

#### Listing Domains

```bash
# List all domains in inventory (including duplicates)
balebos list

# List unique domains only
balebos list --unique
```

#### Viewing Statistics

```bash
# Show basic domain statistics
balebos stats

# Show detailed statistics grouped by domain
balebos stats --by-domain
```

#### Domain Hierarchy Visualization

```bash
balebos tree
```

This command displays domains in a hierarchical tree structure showing:
- Root domains (marked with 🔵)
- Discovered subdomains (marked with ◇)
- Multi-level nested subdomain relationships
- Overall domain structure and organization

Example output:
```
Domain Hierarchy (645)

├── 🔵 example.com
│   ├── ◇ api
│   ├── ◇ www
│   └── ◇ staging
│       └── ◇ api
│
└── 🔵 test.org
    ├── ◇ mail
    └── ◇ dev
```

#### Domain Health Status Report

```bash
balebos status
```

This command provides comprehensive health report of all assets, showing HTTP probe results grouped by status:

**Output includes:**
- 🟢 Alive (HTTP 200) - Domains responding successfully
- 🟡 Client Errors (4xx) - Redirect/auth/not found issues
- 🔴 Server Errors (5xx) - Backend/infrastructure failures
- ⚠️  Network Errors - DNS, timeout, TLS, connection issues
- ⚪ Unchecked - Not yet probed

Each category shows count, percentage, response times, and error details.

#### Exporting Status Reports

Status reports can be exported to multiple formats:

**CSV Export:**
```bash
# Export to default file
balebos status --export csv

# Export to custom file
balebos status --export csv /path/to/report.csv
```

**Markdown Export:**
```bash
# Export to default file
balebos status --export markdown

# Export to custom file
balebos status --export markdown /path/to/report.md
```

**HTML Export:**
```bash
# Export to default file
balebos status --export html

# Export to custom file
balebos status --export html /path/to/report.html
```

**Export features:**
- CSV: Tabular format with all asset details
- Markdown: Summary tables plus detailed sections per category
- HTML: Styled report with color-coded indicators
- All formats include timestamps and complete data

### Analysis and Monitoring

#### Checking for Issues

```bash
balebos check
```

This command analyzes inventory data for:
- Stale domains (not seen in 14+ days)
- Low confidence items (seen only once)
- Unknown data sources
- Normalization issues
- Suspicious subdomain patterns
- HTTP status errors (4xx, 5xx)
- Missing HTTP checks

#### Tracking Changes

```bash
balebos diff
```

Shows changes in the last 7 days:
- New domains/subdomains
- Missing domains/subdomains

#### Consolidated Reports

```bash
balebos report
```

Generates a comprehensive report that includes:
- Inventory statistics
- Recent changes
- Detected issues
- Attention status (whether action is required)

This is the most valuable command for regular operational monitoring.

## Operational Workflow

For effective use of Balebos, we recommend:

1. **Initial Discovery**:
   ```bash
   # Option A: Individual commands
   balebos scan yourdomain.com
   balebos probe subdomains yourdomain.com
   balebos probe http yourdomain.com
   
   # Option B: All-in-one batch command
   balebos batch yourdomain.com
   ```

2. **Daily Health Check**:
   ```bash
   # View domain hierarchy
   balebos tree
   
   # Check domain status
   balebos status
   
   # Full analysis report
   balebos report
   ```

3. **Export for Sharing**:
   ```bash
   # Generate status reports for stakeholders
   balebos status --export html /reports/status.html
   balebos status --export markdown /reports/status.md
   ```

4. **HTTP Status Monitoring**:
   ```bash
   balebos probe http yourdomain.com
   ```

5. **Deep Analysis** (as needed):
   ```bash
   balebos check
   balebos diff
   ```

## Data Storage

All data is stored in the `data/` directory as append-only JSONL files:

### File: `data/inventory.jsonl`

Each line is a JSON object representing a single item in the inventory. Records can be of different types:

**Domain Records:**
```json
{"item_type":"domain","id":"8a594a53-c36c-49f2-9376-34a520e4fa18","name":"example.com","discovered_at":"2026-03-18T19:38:28.958451Z"}
```

**Subdomain Records:**
```json
{"item_type":"subdomain","id":"sub:api.example.com","name":"api.example.com","discovered_at":"2026-03-18T22:45:12.345678Z","source":"subfinder","original_input":"example.com"}
```

**HTTP Check Records:**
```json
{"item_type":"http_check","target":"example.com","status":200,"response_time_ms":156,"checked_at":"2026-03-18T22:45:12.345678Z","error_kind":null,"error_message":null}
{"item_type":"http_check","target":"broken.com","status":null,"response_time_ms":0,"checked_at":"2026-03-18T22:45:12.345678Z","error_kind":"dns_not_found","error_message":"DNS resolution failed"}
```

**Record Types:**
- `domain` - Root domains added to inventory
- `subdomain` - Subdomains discovered via scanning
- `http_check` - HTTP probe results and network error observations

## Error Handling

Balebos is designed for resilience:
- Parse errors for a single line don't crash the entire system
- Invalid data is flagged rather than rejected
- Operations continue despite partial failures

This approach ensures that the system remains operational even when faced with unexpected data or conditions.

## Integration

Balebos can be easily integrated with other tools and systems:

### Cron Jobs

```bash
# Add to crontab
0 9 * * * /path/to/balebos report | mail -s "Daily Infrastructure Report" admin@example.com
```

### CI/CD Pipelines

```yaml
# In your pipeline configuration
steps:
  - name: Infrastructure Check
    command: balebos check
    condition: always()
```

## Filtering with balebos.ignore

Balebos supports non-destructive filtering of noisy or irrelevant assets without modifying stored data.

### Creating an Ignore File

Create a file named `balebos.ignore` in the root directory:

```
# Comment lines are ignored
example-test.com
*.staging.*
dev-*
```

### Supported Pattern Types

- **Exact matches**: Match the entire asset name exactly
- **Glob patterns**: Use `*` as a wildcard to match any characters

### Effect on Commands

Assets matching the patterns in `balebos.ignore` will be:
- Excluded from `check` and `report` commands
- Not counted in statistics or anomaly detection
- Still stored in the inventory (filtering happens only at read time)

### Example

If you have many test domains that trigger warnings but aren't important:

```
# balebos.ignore
*.test.*
*-temp.*
staging-*
```

This will filter them from reports while maintaining the complete data store.

## Extending Balebos

To add new asset types or scanners:

1. Create a new command that collects the data
2. Store it in the inventory with appropriate item_type
3. Update the check logic to handle the new asset type
4. Update reporting to include the new information

The append-only, schema-flexible design makes this extension straightforward.