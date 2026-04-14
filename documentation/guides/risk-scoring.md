# Risk Scoring Guide

Risk scoring in Balebos is the mechanism for understanding which assets need immediate attention. Rather than treating all issues equally, Balebos computes a numerical risk score for each asset based on the findings, observations, and operational context.

## Overview

A risk score is a single number that represents the combined risk from multiple factors:
- Open findings (security issues, outages, missing data)
- Severity levels of those findings
- Asset lifecycle state (active services are higher risk than deprecated ones)
- Observation age (stale data increases uncertainty)
- Missing critical checks

Risk scores help you answer: "If I have 30 minutes before the end of the shift, which five assets should I investigate?"

## Understanding Risk Factors

Risk is computed from multiple signals:

### Findings Contribution

Each open finding contributes points based on severity:

```
Critical finding  → +100 points
High finding      →  +50 points
Medium finding    →  +25 points
Low finding       →  +10 points
Info finding      →   +2 points
```

Multiple findings on the same asset stack additively. An asset with one Critical and two High findings would contribute: 100 + 50 + 50 = 200 points.

### Lifecycle Modifier

The asset's lifecycle state affects risk perception:

```
Active           → ×1.0 (full risk weight)
Under Review     → ×0.7 (risk acknowledged but not critical yet)
Deprecated       → ×0.4 (issues expected, lower priority)
Decommissioned   → ×0.1 (should have no risk, but incomplete removal counts)
Abandoned        → ×1.5 (unknown ownership increases risk)
```

An asset marked Deprecated with a High finding would score: 50 × 0.4 = 20 points instead of 50.

### Observation Age

Assets not observed recently get risk points for uncertainty:

```
Observed today                    → +0 points
Observed 7+ days ago              → +5 points
Observed 14+ days ago             → +15 points
Observed 30+ days ago             → +40 points (usually triggers staleness finding)
Never observed (just added)       → +10 points
```

This reflects the principle that unknown state is risky state.

### Missing Checks

If critical checks haven't been run, uncertainty adds risk:

```
No HTTP check performed           → +10 points
No DNS check performed            → +5 points
No TLS check performed            → +8 points
```

This encourages regular scanning to reduce uncertainty.

## Computing Your Risk Score

The formula is straightforward:

```
Risk Score = (Findings Points + Observation Points + Missing Check Points) × Lifecycle Modifier
```

**Example 1: Production API with issues**
```
Asset: api.example.com
Lifecycle: Active (×1.0)
Findings:
  - 1 Critical (expired TLS cert)     +100
  - 1 High (staleness > 7 days)       +50
Observation age: 3 days               +0
Missing checks: None                  +0
─────────────────────────────────────
Subtotal: 150 × 1.0 = 150 points
Status: URGENT - Needs immediate attention
```

**Example 2: Deprecated service with similar issues**
```
Asset: api-v1.example.com
Lifecycle: Deprecated (×0.4)
Findings:
  - 1 Critical (expired TLS cert)     +100
  - 1 High (staleness > 7 days)       +50
Observation age: 3 days               +0
Missing checks: None                  +0
─────────────────────────────────────
Subtotal: 150 × 0.4 = 60 points
Status: LOW - Can defer until active services are safe
```

Same absolute issues, different operational priority. That's the point.

## Risk Tiers

For quick mental models, scores map to operational priority:

```
0-10:    Green    ✓ No immediate concern
11-50:   Yellow   ⚠ Worth investigating soon
51-150:  Orange   🔴 Needs attention within hours
151+:    Red      🚨 Urgent - investigate immediately
```

These aren't hard rules—your organisation might adjust based on scale and capacity—but they're useful defaults.

## CLI Commands

### Full Risk Report

```bash
balebos risk report
```

Shows all assets ranked by risk score, with a breakdown of contributing factors.

**Output example:**
```
Risk Assessment Report
═════════════════════════════════════════════════════════════

Severity Summary:
  Critical Risk (151+):     3 assets
  High Risk (51-150):       7 assets
  Medium Risk (11-50):      12 assets
  Low Risk (0-10):          48 assets
  ─────────────────────────────────────
  Total Assets:             70

Critical Risk Assets:
───────────────────────────────────────

1. api.example.com                                      Score: 215
   Findings:
     • [Critical] TLS certificate expired 5 days ago
     • [High] Service not seen in 12 days
   Lifecycle: Active
   Observation age: 12 days

2. database.internal.example.com                        Score: 185
   Findings:
     • [Critical] SSH key rotation overdue
     • [High] Unauthorized access attempt detected
   Lifecycle: Active
   Observation age: 2 hours

3. staging.example.com                                  Score: 160
   Findings:
     • [High] SQL injection vulnerability detected
     • [High] Outdated dependencies found
     • [Medium] Missing WAF rules
   Lifecycle: Under Review
   Observation age: 6 hours

High Risk Assets:
───────────────────────────────────────

[... additional assets ...]

Summary Actions:
• 3 assets need immediate investigation (Red tier)
• 2 assets have resolved issues but findings not yet cleared
• 1 asset has abandoned status with unclear ownership
• 12 assets are due for re-scanning (observation age > 7 days)
```

### Top N Highest Risk Assets

```bash
balebos risk top [N]
```

Shows the top N highest-risk assets (default: 10). Perfect for a quick dashboard.

**Example:**
```bash
balebos risk top 5
```

```
Top 5 Highest Risk Assets
═════════════════════════════════════════════════════════════

1. api.example.com                                      215
2. database.internal.example.com                        185
3. staging.example.com                                  160
4. legacy-service.test                                   98
5. admin-panel.example.com                              76
```

### Detailed Risk Analysis for Single Asset

```bash
balebos risk show <asset>
```

Drill down into the risk calculation for a specific asset, showing every contributing factor.

**Example:**
```bash
balebos risk show api.example.com
```

```
Risk Analysis: api.example.com
═════════════════════════════════════════════════════════════

Overall Risk Score: 215 (Red - Urgent)

Findings (150 points):
  • [Critical] TLS certificate expired 5 days ago (created 2026-04-10)
    Severity: Critical
    Category: Security
    Points: 100

  • [High] Domain not observed in 12 days (created 2026-04-03)
    Severity: High
    Category: Lifecycle
    Points: 50

Observation History:
  Last HTTP check: 2026-04-03 (12 days ago)
  Last DNS check:  2026-04-12 (1 day ago)
  Observation age: 12 days
  Points: 0

Missing Checks:
  All critical checks completed
  Points: 0

Lifecycle Context:
  State: Active (×1.0 multiplier)
  Set: 2026-03-01 (Production API)

Calculation:
  (150 + 0 + 0) × 1.0 = 215 points

Recommendations:
  1. Renew TLS certificate immediately (critical blocker)
  2. Run HTTP check to verify service status
  3. After fixes, acknowledge/resolve findings to lower score
```

## Understanding Risk Breakdown

### By Category

```bash
balebos risk report --by-category
```

Shows which types of issues are driving the most risk across your infrastructure.

**Example output:**
```
Risk by Finding Category
═════════════════════════════════════════════════════════════

Security (345 points):
  • TLS certificate expiration (5 assets)
  • Vulnerable dependencies (3 assets)
  • Missing authentication (2 assets)

Lifecycle (298 points):
  • Stale assets not observed (8 assets)
  • Assets in unknown state (2 assets)

Operational (156 points):
  • HTTP errors (4 assets)
  • DNS resolution failures (2 assets)

Compliance (89 points):
  • Missing access logs (3 assets)
  • Unencrypted data transmission (1 asset)
```

This helps you prioritise remediation efforts organisation-wide.

### By Lifecycle State

```bash
balebos risk report --by-lifecycle
```

Helps you understand whether your risk is concentrated in Active services (concerning) or Deprecated ones (expected).

```
Risk by Lifecycle State
═════════════════════════════════════════════════════════════

Active (892 points, 38 assets):
  • Requires attention - production issues are high-priority
  • Trending: +15 points since last week

Deprecated (234 points, 12 assets):
  • Elevated due to stale observations
  • Trending: -50 points (good - assets being cleaned up)

Abandoned (145 points, 3 assets):
  • Ownership gaps are concerning
  • Action: Clarify ownership or decommission

Decommissioned (28 points, 1 asset):
  • Near-zero risk, cleanup nearly complete
```

## Integration with Other Commands

### Findings to Risk

Every open finding flows directly into risk scores. Resolve a Critical finding:

```bash
balebos finding resolve <id>
balebos risk show <asset>  # Score drops
```

### Lifecycle Changes

Changing an asset's state immediately changes its risk perception:

```bash
balebos asset lifecycle set api-v1.example.com deprecated --reason "Sunset planned Q3"
balebos risk show api-v1.example.com  # Score drops due to lifecycle modifier
```

### Organisational Context

Assets without project ownership get governance findings, increasing risk:

```bash
balebos risk show orphaned-service.example.com
# Shows governance findings + abandoned status = elevated risk
```

## Real-World Workflows

### Daily Triage

```bash
# Check in the morning
balebos risk top 10

# Deep dive on any red-tier assets
balebos risk show api.example.com
balebos finding list --target api.example.com

# Resolve issues, findings auto-close
balebos finding run
balebos risk show api.example.com  # Score improved
```

### Incident Response

```bash
# Identify affected assets
balebos risk report --by-category | grep -A5 "Security"

# Get details
balebos risk show <critical-asset>
balebos finding list --target <critical-asset> --severity critical

# Track remediation
balebos asset lifecycle set <asset> under-review --reason "Critical incident, in mitigation"
balebos risk show <asset>  # Lower while working on it
```

### Compliance Reporting

```bash
# Show risk summary for stakeholders
balebos risk report > risk-report.txt

# Highlight progress
balebos risk report --by-category  # Show which categories are improving

# Archive for audit
balebos risk report --export json > risk-2026-04-15.json
```

### Capacity Planning

```bash
# How many red-tier assets need attention?
balebos risk report | grep "Red"

# Is our remediation keeping up with new issues?
balebos risk report --trend 7d  # Compare to last week

# Which teams own the highest-risk assets?
balebos risk report --with-owner
```

## Tuning Risk Scores

Most organisations use the default factors, but Balebos supports customisation:

### Adjusting Severity Weights

If you think Medium findings are more serious in your environment:

```bash
# Edit configuration (future feature)
balebos config set risk.medium-weight 35  # default: 25
```

### Customising Lifecycle Modifiers

If your Deprecated assets are still business-critical:

```bash
balebos config set risk.lifecycle.deprecated 0.8  # default: 0.4
```

### Observation Recency Thresholds

If you scan less frequently and want to reduce observation-age anxiety:

```bash
balebos config set risk.stale-threshold-days 30  # default: 14
```

## Data Format

Risk scores are computed at query time from findings, observations, and asset state. They're not stored—they're calculated every time you run `balebos risk report`, ensuring fresh data.

This means:
- No staleness in the risk assessment
- Risk scores adapt immediately when you resolve findings
- No background job needed to recalculate

## See Also

- [Findings Management](findings.md) - Understanding findings that feed risk
- [Asset Lifecycle](asset-lifecycle.md) - How lifecycle state affects risk
- [Organisation Model](organization.md) - Ownership context for risk
- [Architecture](../architecture.md) - How risk scoring fits into the system
