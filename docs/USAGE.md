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

1. **Daily Collection**:
   ```bash
   balebos scan yourdomain.com
   balebos probe subdomains yourdomain.com
   ```

2. **Daily Monitoring**:
   ```bash
   balebos report | mail -s "Balebos Daily Report" your@email.com
   ```

3. **Deep Analysis** (as needed):
   ```bash
   balebos check
   balebos diff
   ```

## Data Storage

All data is stored in the `data/` directory as append-only JSONL files:

### File: `data/inventory.jsonl`

Each line is a JSON object representing a single item in the inventory:

```json
{"item_type":"domain","id":"8a594a53-c36c-49f2-9376-34a520e4fa18","name":"example.com","discovered_at":"2026-03-18T19:38:28.958451Z"}
{"item_type":"subdomain","id":"sub:api.example.com","name":"api.example.com","discovered_at":"2026-03-18T22:45:12.345678Z","source":"subfinder","original_input":"example.com"}
```

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

## Extending Balebos

To add new asset types or scanners:

1. Create a new command that collects the data
2. Store it in the inventory with appropriate item_type
3. Update the check logic to handle the new asset type
4. Update reporting to include the new information

The append-only, schema-flexible design makes this extension straightforward.