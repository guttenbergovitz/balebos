# Asset Lifecycle Management Guide

Asset lifecycle management allows you to formally track the operational status of each asset (domain, subdomain, service) in your inventory. Rather than just collecting data, you can mark assets with explicit states that signal their operational intent.

## Overview

Every asset moves through a formal lifecycle. You can set, query, and track these states through the CLI and TUI. The lifecycle provides context for findings, risk scoring, and operational decision-making.

## Lifecycle States

### Active

The asset is in active production use. It's expected to be regularly monitored, findings should be investigated promptly, and risk scores are meaningful.

**Use when:**
- Asset is live and actively serving users/systems
- Asset is part of the current infrastructure
- Regular monitoring is appropriate

**Example:**
```bash
balebos asset lifecycle set api.example.com active
balebos asset lifecycle set www.example.com active --reason "Production web server"
```

### Under Review

The asset is under evaluation or planning. It might be a new service in testing, scheduled for deployment, or under architectural review.

**Use when:**
- Preparing to launch a new service
- Evaluating a new domain or API endpoint
- In pre-production testing phase
- Making deployment decisions

**Findings behaviour:**
- Findings are still generated but suppressed from high-priority lists
- Auto-resolution happens normally
- Useful for catching issues before production goes live

**Example:**
```bash
balebos asset lifecycle set staging-api.example.com under-review --reason "Staging environment for v2 API"
```

### Deprecated

The asset is being phased out but still operational. It's receiving no new investment and users are being migrated away. It still exists because removing it would break things.

**Use when:**
- Old API versions being sunset
- Legacy services with active consumers
- Scheduled for decommissioning in future quarters
- Infrastructure being consolidated

**Findings behaviour:**
- Findings generate normally but are marked as non-critical
- Missing or broken services are expected in this state
- Auto-resolution still happens
- Useful for planning decommissioning work

**Example:**
```bash
balebos asset lifecycle set api-v1.example.com deprecated --reason "Legacy API, users migrated to v2"
```

### Decommissioned

The asset has been shut down. No one should be relying on it. It's deleted from production but may still appear in DNS records or caches.

**Use when:**
- Service has been turned off
- Domain no longer resolves
- Infrastructure has been removed
- Waiting for DNS caches to clear

**Findings behaviour:**
- Finding generation may be suppressed or marked as informational
- Network errors and missing services are expected
- Useful for tracking cleanup work completion
- Remains in inventory for historical record

**Example:**
```bash
balebos asset lifecycle set old-service.example.com decommissioned --reason "Shut down 2026-04-10, DNS still resolving"
```

### Abandoned

The asset exists in your infrastructure but its ownership, purpose, and operator are unknown. It may be a forgotten service, a test deployment left behind, or a rogue asset.

**Use when:**
- Asset discovered but has no owner
- Purpose and context are unclear
- No documentation exists
- Requires investigation before any action

**Findings behaviour:**
- Generates governance findings requesting clarification
- Higher importance in risk assessment
- Useful for security and compliance teams
- Signals need for inventory cleansing

**Example:**
```bash
balebos asset lifecycle set mystery-subdomain.example.com abandoned --reason "Found during subdomain scan, no known owner"
```

## CLI Commands

### Set Asset Lifecycle State

```bash
balebos asset lifecycle set <asset> <state> [--reason <text>]
```

Set the lifecycle state of an asset with optional reason (useful for future context).

**Examples:**
```bash
# Set to active
balebos asset lifecycle set api.example.com active

# Set with reason
balebos asset lifecycle set legacy-api.example.com deprecated --reason "Sunset scheduled for Q3 2026"

# Mark as abandoned pending investigation
balebos asset lifecycle set unknown-service.test abandoned --reason "Discovered in scan, under investigation"
```

### Show Asset Lifecycle Info

```bash
balebos asset lifecycle show <asset>
```

Display the current lifecycle state, when it was set, and any reason provided.

**Output example:**
```
Asset: api.example.com
State:        Active
Set at:       2026-04-12 10:15:00 UTC
Reason:       Production API endpoint
Last updated: 2026-04-12 10:15:00 UTC
```

### Show Lifecycle History

```bash
balebos asset lifecycle history <asset>
```

View the complete state transition history for an asset, showing all changes and their reasons.

**Output example:**
```
api.example.com - Lifecycle History

2026-04-12 10:15:00 UTC → Active
  Reason: Production API endpoint

2026-04-10 14:30:00 UTC → Under Review
  Reason: Testing new endpoint

2026-04-08 09:00:00 UTC → Active
  Reason: Previously active
```

### List Assets by Lifecycle State

```bash
balebos asset lifecycle list
```

Shows all assets grouped by their lifecycle state, useful for understanding your infrastructure status.

**Output example:**
```
Active (23):
  api.example.com
  www.example.com
  cdn.example.com
  ...

Deprecated (4):
  api-v1.example.com
  old-service.example.com
  ...

Abandoned (2):
  mystery-subdomain.example.com
  unknown-service.test

Decommissioned (1):
  legacy-db.example.com
```

Filter by state:
```bash
balebos asset lifecycle list --state active
balebos asset lifecycle list --state abandoned
balebos asset lifecycle list --state deprecated
```

## TUI Integration

The interactive dashboard includes lifecycle management:

1. **View lifecycle state** - Each asset shows its current state in the inventory table
2. **Set state** - Press `l` on an asset to open the lifecycle picker
3. **Add reason** - Modal prompts for optional reason/notes
4. **History** - Press `h` to view state transitions
5. **Filter by state** - Use filter bar to show assets in specific lifecycle state

## Risk Scoring Impact

Lifecycle state directly affects risk scores and findings prioritisation:

- **Active** - Issues are high-priority, risk scores are meaningful
- **Under Review** - Issues noted but suppressed from critical alerts
- **Deprecated** - Expected issues are lower-severity, non-critical
- **Decommissioned** - Service problems are not security issues
- **Abandoned** - Governance issues are elevated, security risk is unclear

Example: An HTTP 500 error on an Active service might be Critical severity, but the same error on a Deprecated service might be Low severity. The system understands operational context.

## Data Storage

Lifecycle state is stored as append-only records in Parquet:

**Lifecycle state record:**
```json
{
  "item_type": "asset_lifecycle",
  "target": "api.example.com",
  "state": "Active",
  "reason": "Production API endpoint",
  "changed_at": "2026-04-12T10:15:00Z"
}
```

The current state is computed by finding the most recent record for each asset. Full history is preserved.

## Common Workflows

### Launching a New Service

```bash
# Initially mark as under review
balebos asset lifecycle set new-api.example.com under-review --reason "Pre-launch testing"

# Run checks to catch issues before launch
balebos finding run
balebos finding list --severity high

# After launch, mark as active
balebos asset lifecycle set new-api.example.com active --reason "Launched 2026-04-15"
```

### Deprecating a Service

```bash
# Mark when deprecation is announced
balebos asset lifecycle set api-v1.example.com deprecated --reason "Sunset planned for 2026-Q3, users should migrate to v2"

# Monitor the transition
balebos asset lifecycle history api-v1.example.com

# When fully decommissioned
balebos asset lifecycle set api-v1.example.com decommissioned --reason "Shut down 2026-07-01"
```

### Inventory Cleanup

```bash
# Find unknown assets
balebos asset lifecycle list --state abandoned

# Investigate
balebos risk show unknown-service.test

# Mark once ownership is clarified
balebos asset lifecycle set unknown-service.test active --reason "Owned by Platform team, production CI/CD infrastructure"
```

## Integration with Other Features

### With Findings

- Findings generation respects lifecycle state
- Deprecated/Decommissioned service errors don't escalate
- Abandoned assets generate governance findings

### With Organisations

- Assets in projects should be marked as Active if project is active
- Decommissioned projects should have all assets marked Decommissioned
- Abandoned assets may indicate missing project assignments

### With Ignore Patterns

- Deprecated/Decommissioned assets can be filtered from reporting without deletion
- Allows gradual removal from active monitoring
- Preserved for historical analysis

## Example: Multi-Environment Management

```bash
# Production - Active
balebos asset lifecycle set api.production.example.com active --reason "Production environment"

# Staging - Under Review
balebos asset lifecycle set api.staging.example.com under-review --reason "Staging for testing releases"

# Legacy production - Deprecated
balebos asset lifecycle set api-old.example.com deprecated --reason "Legacy version, sunset in Q3"

# Disconnected test - Abandoned
balebos asset lifecycle set test-old-unused.example.com abandoned --reason "Old test environment, no known owner"

# Previous staging - Decommissioned
balebos asset lifecycle set api.staging-old.example.com decommissioned --reason "Shut down when new staging provisioned"
```

## See Also

- [Findings Management](findings.md) - How lifecycle state affects findings
- [Risk Scoring](risk-scoring.md) - How lifecycle impacts risk assessment
- [Organisation Model](organization.md) - Linking assets to projects and ownership
- [Data Model](../specs/data-model.md) - Storage schema for lifecycle records
