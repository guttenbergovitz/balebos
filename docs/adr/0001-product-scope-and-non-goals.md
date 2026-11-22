# 0001 — Product scope and explicit non-goals

**Status:** Accepted

## Context

Balebos 1.x grew by accretion: discovery, probing, findings, risk scores, an
organisational governance subsystem and a web dashboard, with scope drifting
towards both security scanning (Angry Turnip's territory) and enterprise CMDB
functionality. Without an explicit boundary, every refactoring decision becomes
debatable.

## Decision

Balebos is a local-first infrastructure discovery, inspection and drift
detection tool. It owns: domain/subdomain discovery, DNS inspection, HTTP(S)
probing, redirect inspection, TLS observations, RDAP observations, ASN and
hosting attribution, fingerprinting, local inventory, history, snapshots, drift
detection, findings/hygiene checks, CLI/TUI inspection and structured exports.

Explicit non-goals: software security scanning (SCA, CVE/CVSS/KEV/EPSS
enrichment, secret scanning, SBOM, licence compliance), enterprise governance
(CMDB authority, org hierarchies, approvals, risk acceptance) and distributed
infrastructure (servers, brokers, central databases). Optional integrations must
never be prerequisites for local operation.

## Consequences

- Every feature proposal is tested against the five fundamental questions in
  `docs/product.md`.
- The organisational subsystem is a removal candidate (ADR 0008); aggregate risk
  scoring is a simplification candidate (assessment engine redesign, Phase 4).
- Angry Turnip remains the single owner of software security assessment;
  cross-references between the tools are integration contracts, not feature merges.
