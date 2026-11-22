# 0002 — Separation from Angry Turnip and higher-level systems

**Status:** Accepted

## Context

Balebos and Angry Turnip are sibling tools in the same ecosystem, and Balebos
1.x documentation blurred their boundaries ("control plane", "risk report").
Likewise, 1.x absorbed Brand → Company → Project → Person governance that
belongs in enterprise systems (CMDB, IAM, ITSM), duplicating data that such
systems authoritatively own.

## Decision

Hard boundaries, enforced by review:

- **Angry Turnip owns** software composition analysis, vulnerability scanning,
  CVE/CVSS/KEV/EPSS enrichment, secret scanning, SBOMs, licence compliance and
  repository security assessment. Balebos must not reimplement any of it. It may
  record that an endpoint exposes a software fingerprint; it does not assess that
  software's vulnerabilities.
- **Higher-level systems own** CMDB authority, organisational hierarchy, people
  and roles, approvals and business-risk acceptance. Balebos keeps only
  lightweight local metadata: tags, notes, asset importance and owner references
  as plain data useful for standalone work.

Integration with either world happens through explicit, versioned contracts
(ADR 0010) that preserve original sources and uncertainty.

## Consequences

- The `org` command family and its storage records are retired (ADR 0008), with
  data export before removal (`docs/migration-v2.md`).
- No dependency of Balebos may require a central service to run locally.
- The `balebos-architecture-review` skill checks for product-scope drift at every
  review.
