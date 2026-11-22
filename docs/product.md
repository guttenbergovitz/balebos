# Balebos product definition

## Identity

Balebos is a **local-first infrastructure discovery, inspection and drift
detection tool**. It helps operators discover, understand and track the externally
observable state of their digital infrastructure without requiring a central
server, managed database or monitoring platform.

Infrastructure decays: domains are forgotten, DNS records outlive migrations,
certificates expire, hosting changes, redirects break, old endpoints remain
accessible. Monitoring known services is not equivalent to knowing what
infrastructure actually exists. Balebos closes that visibility gap.

## Fundamental questions

Every feature must meaningfully contribute to at least one of:

1. **What exists?** Domains, subdomains, hosts and observable endpoints.
2. **What is running?** Responding services, behaviour, technologies, providers.
3. **What has changed?** Appearances, disappearances, configuration or behaviour changes.
4. **What needs attention?** Operational problems, drift, deteriorating hygiene.
5. **What evidence supports these conclusions?** When, by which tool, under what
   conditions, with what result.

## In scope (Balebos owns)

- Domain and subdomain discovery; DNS inspection; HTTP/HTTPS probing; redirect
  inspection; TLS certificate observations; RDAP/registration observations; ASN
  and network attribution; hosting and technology fingerprinting.
- Local inventory, historical observations and snapshots, drift detection,
  operational findings and hygiene checks.
- CLI and TUI inspection interfaces; structured exports for external systems.

## Out of scope (explicit non-goals)

- Software security scanning (SCA, CVE/CVSS/KEV/EPSS, secrets, SBOM, licence
  compliance, Dockerfile/CI auditing): **Angry Turnip's** territory. Balebos may
  record that an endpoint exposes a server version; it does not become a CVE
  scanner for that software.
- Enterprise governance: CMDB authority, organisational hierarchies, people and
  role administration, approval processes, business-risk acceptance. Lightweight
  local grouping, tags, notes, importance and owner references are retained where
  useful for standalone work.
- Distributed orchestration, event buses, central API servers, authentication
  infrastructure. Integrations with CMDBs or observability platforms are optional
  and never prerequisites for local operation.

## Promises and honesty

Balebos must not promise perfect discovery, complete ownership attribution or
continuous monitoring where those cannot be demonstrated. A failed discovery pass
never implies previously known assets disappeared. A partial scan is always
identifiable as partial. Findings are deterministic assessments over persisted
evidence, and each finding must be explainable: which rule, which observations,
what is missing, whether the scan was complete.
