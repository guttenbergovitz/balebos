# Balebos — Wish List

Status: living document, 2026-04-12

What we want Balebos to become, in plain language. Not a spec, not a roadmap — just the full picture of what "done" looks like. Items are grouped by theme, not priority. See [evolution-roadmap.md](./evolution-roadmap.md) for sequencing.

---

## Know what we have

- [ ] Register any digital asset, not just domains: APIs, repositories, cloud resources, SaaS dependencies, certificates, campaigns, hosting environments, vendor-owned systems
- [ ] Every asset has an owner, a brand, a country, a cost centre
- [ ] Every asset has a lifecycle state: active, under review, deprecated, decommissioned, abandoned
- [ ] Assets that have no owner are surfaced automatically — not buried in a report
- [ ] The system knows which assets were discovered automatically and which were registered manually
- [ ] Extensible metadata per asset type (a repo has a language and a remote URL; a cloud resource has a provider and region; a domain has a registrar and expiry)

## Know what's happening

- [ ] Broad, shallow scanning across the entire estate — DNS, HTTP, TLS, WHOIS/RDAP, tech fingerprinting, hosting detection
- [ ] Observations are append-only, timestamped, source-tagged — never overwritten
- [ ] Historical record of every observation, queryable over time ("show me TLS state for example.com since January")
- [ ] Heterogeneous scanners: Go binaries, Node scripts, shell scripts, API calls, Playwright runners — described by manifests, not compiled in
- [ ] Scheduled checks: DNS hourly, RDAP weekly, vulnerability nightly, accessibility monthly — each on its own cadence
- [ ] Event-driven checks: post-commit hooks, post-deploy triggers, "something changed, go look deeper"
- [ ] Timeout, retry, and failure isolation per scanner — one hanging tool doesn't block everything

## Know what's wrong

- [ ] Findings as first-class entities: ID, severity, confidence, category, affected asset, linked evidence, lifecycle (open → acknowledged → assigned → resolved → reopened)
- [ ] Separate signal categories: security, compliance, financial, lifecycle, operational, governance
- [ ] Risk scoring per asset: weighted aggregation of active findings
- [ ] Drift detection: "response time doubled this week", "tech stack changed from Nginx to Apache", "TLS issuer changed unexpectedly"
- [ ] Noise management: distinguish scanner noise from genuine issues, track false positive rates per check type
- [ ] Confidence levels on observations and findings — not everything is equally trustworthy

## Know what's decaying

- [ ] Stale assets: no recent observations, no owner activity, no traffic
- [ ] Zombie costs: domains auto-renewing without business justification, services consuming budget with zero value
- [ ] Abandoned systems: still online, possibly still processing personal data, but nobody's watching
- [ ] End-of-life frameworks and dependencies
- [ ] Expired or soon-to-expire certificates and domains
- [ ] Repositories with no commits in 6+ months

## Know what changed

- [ ] Change log: what appeared, what disappeared, what shifted
- [ ] Diff view: compare estate state between any two points in time
- [ ] Alerts on meaningful changes, not every DNS TTL fluctuation
- [ ] Audit trail: prove to an auditor that asset X was monitored continuously from date A to date B

## Let AI help — carefully

- [ ] Selective, tool-using AI agents for ambiguous cases: suspicious redirects, unclear vulnerabilities, noisy scan results, abandoned-or-not judgement calls
- [ ] AI runs only when triggered by a specific finding pattern — not as the primary scanning layer
- [ ] Agent output is a finding with `requires_review: true` — never automated enforcement
- [ ] Cost control: budget per investigation, daily/weekly cap
- [ ] Full audit trail of agent invocations, tool calls, reasoning, conclusions
- [ ] Clear separation: deterministic checks produce facts, AI produces opinions that humans act on

## Let humans act

- [ ] HTTP API: inventory, observations, findings, org data — read and write
- [ ] Web dashboard for non-technical stakeholders: compliance officers, finance, management
- [ ] TUI stays as the operator's power tool
- [ ] Triage workflow: acknowledge, assign, resolve, escalate — directly from any interface
- [ ] Scheduled reports: weekly summary, monthly compliance snapshot, quarterly cost review
- [ ] Export: CSV, Markdown, HTML, JSON — for integration with whatever else the org uses

## Stay boring

- [ ] Single binary, feature-gated optional backends
- [ ] Pure Rust where possible — no C/C++ dependencies on the critical path
- [ ] JSONL as the universal interchange format, always available
- [ ] Parquet + DataFusion as the analytical backend (replacing DuckDB)
- [ ] No microservices until proven necessary
- [ ] No external message bus until in-process channels prove insufficient
- [ ] Minimal config: works with defaults, tuneable when needed
- [ ] Compiles and runs on Mac, Linux x86, Linux ARM, in Docker, without special toolchains

## Survive the organisation

- [ ] Works when ownership is unclear (that's the default state, not an edge case)
- [ ] Works when agencies resist standardisation (they always do)
- [ ] Works when nobody looks at it for a month (alerts and findings accumulate, nothing rots silently)
- [ ] Works when the team is small (one person can operate the whole system)
- [ ] Works when stakeholders don't understand tech (the dashboard speaks their language)
- [ ] Doesn't become another stale dashboard — active findings require action, not just display
