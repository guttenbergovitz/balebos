# Findings Management Guide

Findings are persisted issues detected by Balebos checks. Unlike ephemeral reports that disappear when the command finishes, findings are stored, queryable, and have a proper lifecycle.

## Overview

A finding captures an operational problem your infrastructure scan has detected. It gets a unique ID, severity level, category, and persists until you resolve it or the underlying issue fixes itself.

Common finding types:
- Domain hasn't been seen recently (staleness)
- TLS certificate expiring soon or already expired
- HTTP service returning errors (5xx, network timeouts)
- Low confidence due to too few observations
- Assets in unknown organisational context

## Severity Levels

Findings are prioritised by severity:

- **Critical** - Requires immediate action. Expired certificates, service outages, security breaches.
- **High** - Address soon. Certificates expiring within days, stale production assets, policy violations.
- **Medium** - Worth investigating. Low observation count, suspicious patterns, missing data.
- **Low** - Minor issues. Missing optional checks, cosmetic problems.
- **Info** - Informational only. Not a problem, just worth noting.

## Categories

Findings are organised by category to support different workflows:

- **Security** - Certificate issues, TLS problems, cryptographic weaknesses
- **Compliance** - Policy violations, governance gaps, missing controls
- **Financial** - Cost-related concerns, unused resources, waste
- **Lifecycle** - Stale assets, deprecation warnings, decommissioning
- **Operational** - HTTP errors, DNS issues, connectivity problems
- **Governance** - Ownership gaps, tagging issues, context missing

## Lifecycle

A finding follows a formal lifecycle with four states:

```
Created → Open → Acknowledged → Resolved
   ↓
Auto-resolved (when condition is fixed)
```

### State Descriptions

**Created** - Check detected an issue. If no finding exists for this check+target combination, it's created immediately. Duplicates are ignored (same check + same asset = no new finding).

**Open** - Default state. Requires human attention. Triggers alerts in the `balebos risk report`.

**Acknowledged** - You've seen it and understand it. Suppresses alerts whilst the underlying issue is being fixed. Useful for things you're already working on.

**Resolved** - Issue is fixed. Can happen two ways:
- Manually: You run `balebos finding resolve <id>`
- Automatically: The underlying condition is fixed and the next check run clears it

## Auto-Resolution

Findings resolve automatically when the problem goes away:

- **Staleness** - Finding closes when asset is seen again
- **HTTP error** - Finding closes when service returns 2xx status
- **Low confidence** - Finding closes once multiple observations exist
- **Expiring certificate** - Finding closes when certificate renews
- **Missing data** - Finding closes when data appears

This means you don't need to manually close findings. The system keeps itself clean.

## CLI Commands

### Run checks and persist findings

```bash
balebos finding run
```

Executes all configured checks against your inventory. Creates findings for detected issues. Existing findings for the same check+target are skipped (deduplication). Auto-resolves findings when conditions improve.

### List findings

```bash
balebos finding list
```

Shows all findings with status, severity, category, target, and message.

Filter by status:
```bash
balebos finding list --status open
balebos finding list --status acknowledged
balebos finding list --status resolved
```

Filter by severity:
```bash
balebos finding list --severity high
balebos finding list --severity critical
balebos finding list --severity medium
```

Combine filters:
```bash
balebos finding list --status open --severity high
```

### Show finding details

```bash
balebos finding show <id>
```

Displays full finding information including:
- Created timestamp
- Lifecycle events (acknowledges, resolutions)
- Auto-resolution details if applicable
- Full check output and evidence

### Acknowledge a finding

```bash
balebos finding acknowledge <id>
```

Marks finding as "Acknowledged". Suppresses alerts but keeps it visible. Use this when:
- You understand the issue
- You're actively working on a fix
- You want to suppress noise temporarily

### Resolve a finding

```bash
balebos finding resolve <id>
```

Manually marks a finding as resolved. Use when:
- You've fixed the underlying issue but the auto-resolution hasn't run yet
- The finding is a false positive
- You want to close it without fixing the issue

### Clear all resolved findings

```bash
balebos finding clear-resolved
```

Deletes all findings in resolved state. Useful for housekeeping after batch fixes.

## TUI Integration

The interactive dashboard includes a dedicated Findings tab:

- Browse all findings with sorting and filtering
- View findings by status (Open, Acknowledged, Resolved)
- Filter by severity to focus on critical issues
- Quick actions: acknowledge, resolve, show details
- Live updates as `balebos finding run` executes

Navigate with arrow keys, filter with `/`, and press `?` for keyboard help.

## Integration with Risk Scoring

Findings feed directly into the risk scoring system. Open findings with:
- Critical severity contribute maximum risk points
- High severity contribute significant points
- Medium and Low contribute lesser amounts

The `balebos risk report` command shows which findings are driving risk for each asset. Resolving findings lowers risk scores.

## Example Workflow

1. Run initial scan and checks:
   ```bash
   balebos scan example.com
   balebos probe subdomains example.com
   balebos probe http example.com
   balebos finding run
   ```

2. Review findings:
   ```bash
   balebos finding list --status open --severity high
   ```

3. Triage—decide what needs immediate attention:
   ```bash
   balebos risk report          # See which assets are problematic
   balebos finding show <id>    # Examine specific findings
   ```

4. Act on findings—fix the underlying issues, then acknowledge or resolve:
   ```bash
   balebos finding acknowledge <id>   # Suppress alert, keep tracking
   balebos finding resolve <id>       # Mark as fixed
   ```

5. Re-run checks to auto-resolve fixed issues:
   ```bash
   balebos finding run
   ```

## Deduplication Logic

Findings are deduplicated by the `check_name` + `target` combination. If:
- Check "staleness" finds `example.com` hasn't been seen in 30 days
- An open finding already exists for staleness + example.com
- A new finding is NOT created

This prevents finding spam from repeated check runs. Only the first detection creates a finding; subsequent runs just skip it.

## Storage and Audit

Findings are stored as append-only records in Parquet storage:
- Each finding gets a UUID for stable reference
- Lifecycle events (acknowledge, resolve) are stored separately
- Full audit trail is preserved—you can trace every state change
- Finding status is computed by querying events, not by storing mutable state

This means:
- No lost history if someone accidentally resolves a finding
- Complete audit trail for compliance
- Resilience to failures during state transitions

## Data Format

Findings are stored as `finding` and `finding_event` records:

**Finding record:**
```json
{
  "item_type": "finding",
  "id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "severity": "High",
  "category": "Lifecycle",
  "check_name": "staleness",
  "target": "api.example.com",
  "message": "Domain not seen in 21 days (threshold: 14 days)",
  "created_at": "2026-04-12T10:30:00Z"
}
```

**Event record (acknowledge):**
```json
{
  "item_type": "finding_event",
  "id": "event:f47ac10b-58cc-4372-a567-0e02b2c3d479:1712922600",
  "finding_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "event_type": "Acknowledged",
  "created_at": "2026-04-12T11:00:00Z"
}
```

See [Data Model](../specs/data-model.md) for complete schema documentation.

## Troubleshooting

### Findings not auto-resolving

Auto-resolution happens when you run `balebos finding run`. Make sure:
- You're running checks regularly (cron job, scheduled task, etc.)
- The underlying condition is actually fixed
- There's no conflicting open finding for a related issue

### Duplicate findings appearing

If you're seeing duplicate findings with slightly different timestamps, check:
- Are you running `balebos finding run` multiple times in quick succession?
- Are different check rules creating findings for the same issue?

Consider adjusting check thresholds or consolidating related checks.

### Too many low-severity findings

If noise is overwhelming, try:
- Filtering by severity: `balebos finding list --severity high --status open`
- Acknowledging non-critical findings you're not addressing
- Adjusting check thresholds to reduce false positives

## See Also

- [Risk Scoring Guide](risk-scoring.md) - How findings feed into risk calculation
- [Data Model](../specs/data-model.md) - Detailed schema for findings storage
- [Architecture](../architecture.md) - How findings fit into the system design
