# Balebos — Big Picture

Status: architectural planning document, 2026-04-12

This document describes what Balebos is meant to become: a control and observability system for digital assets across a disorganised enterprise. It covers the problem, the domain, the architecture, and the decisions that need to be made.

Related documents:
- [Gap analysis](./gap-analysis.md) — what we have today vs what's needed
- [Evolution roadmap](./evolution-roadmap.md) — phased delivery plan
- [Wish list](./wishlist.md) — feature-level checklist
- [DuckDB concerns](./duckdb-concerns.md) — storage platform decision

---

## 1. Executive summary

Balebos is a control plane for digital assets. Not a dashboard, not a SIEM, not a vulnerability scanner. It's the system that answers: "what do we have, who owns it, is it healthy, and is it worth paying for?"

The organisation it serves has many digital assets spread across brands, countries, agencies, projects, and historical leftovers. There is organisational chaos: unclear ownership, legacy websites, domains that auto-renew silently, external vendors with their own standards, inconsistent technology choices, and weak visibility into status, freshness, security, and cost.

Most systems don't fail loudly. They decay. Balebos exists to make that decay visible and controlled.

The true complexity of the system does not lie in running scanners or building dashboards. It lies in:
- **the domain model**: what is an asset, who owns it, what's its lifecycle, what observations exist, what do they mean
- **the interpretation layer**: converting raw observations into findings, risk signals, and actionable recommendations
- **the organisational reality**: assets without owners, agencies that resist standardisation, stakeholders who never look at dashboards

The system should be optimised for durability, clarity, explainability, maintainability, incremental adoption, and operational boringness. It should survive years, not just a demo.

What should not be overdesigned early: orchestration infrastructure, AI pipelines, web dashboards, plugin systems, event buses. These can be added incrementally once the domain model and storage are solid.

---

## 2. Stakeholder map

### Security team
- **Questions**: Which public assets are vulnerable? Which vulnerabilities are real vs scanner noise? Which repos introduced new risk? What changed since the last deployment?
- **Outputs**: Prioritised finding list, severity trends over time, evidence for remediation tickets.
- **Decisions**: What to patch, what to escalate, what to accept.

### Compliance / DPO
- **Questions**: Which assets have no owner? Which abandoned assets may still process personal data? Can we prove continuous monitoring for an audit? Which certificates were expired, for how long, and who was responsible?
- **Outputs**: Audit-ready historical evidence, ownership gap reports, data processing inventory.
- **Decisions**: What to decommission, what to investigate, what to document for regulators.

### Finance / controlling
- **Questions**: Which domains are auto-renewing without justification? Which services consume budget with zero traffic? Which zombie systems exist only because nobody disabled them?
- **Outputs**: Cost-per-asset breakdown, unused asset list, renewal calendar.
- **Decisions**: What to cancel, what to consolidate, what to challenge.

### Platform / ops team
- **Questions**: What failed today? What degraded vs yesterday? Is it transient or structural? Which checks are trustworthy?
- **Outputs**: Operational status overview, alert feed, scanner health metrics.
- **Decisions**: What to investigate, what to restart, what to ignore.

### Management / leadership
- **Questions**: What's our overall risk posture? Are we improving or degrading? Which business units have the worst hygiene? What requires escalation?
- **Outputs**: Executive summary, trend charts, risk heatmap by brand/country.
- **Decisions**: Where to invest, what to escalate, who to hold accountable.

### Content / marketing
- **Questions**: Which campaigns should have been retired but are still online? Which landing pages are broken? Which microsites are unmaintained?
- **Outputs**: Campaign asset inventory, health status, retirement candidates.
- **Decisions**: What to take down, what to refresh, what to redirect.

### Vendor governance
- **Questions**: Which assets are managed by external agencies? Are they maintaining them? Are their systems up to date? Are they meeting SLAs?
- **Outputs**: Vendor asset inventory, health comparison across vendors, SLA evidence.
- **Decisions**: Which vendors to challenge, which contracts to renegotiate.

### Asset owners (distributed across org)
- **Questions**: What do I own? Is it healthy? What do I need to act on?
- **Outputs**: Per-owner asset summary, open findings, recommended actions.
- **Decisions**: What to fix, what to delegate, what to decommission.

---

## 3. Domain model

### Core entities

**Asset.** A digital thing the organisation is responsible for. Has an identity, a type, an owner (possibly unknown), an organisational mapping (brand → company → project), and a lifecycle state.

Asset types include: domain, subdomain, website, web application, API, repository, cloud resource, certificate, DNS configuration, campaign / landing page, vendor-owned asset, SaaS dependency, hosting environment. These are data-driven categories, not separate Rust types.

**Observation.** A point-in-time fact about an asset, captured by a scanner or check. Append-only, never overwritten. Has: target asset, observation type, timestamp, source, confidence level, and a typed payload.

Observations are not state. They are evidence. The current state of an asset is derived from its most recent observations, not stored as a mutable field.

**Finding.** An interpreted signal derived from one or more observations. Has: ID, severity, confidence, category (security / compliance / financial / lifecycle / operational / governance), affected asset, linked observations, and a lifecycle (open → acknowledged → assigned → resolved → reopened).

Findings are the primary output of the interpretation layer. They are what stakeholders act on.

### Identity vs observation

This is the most important architectural boundary in the system.

- **Identity layer**: what exists, who owns it, what's its lifecycle state. Mutable (lifecycle transitions), but changes are recorded as events.
- **Observation layer**: what was seen, when, by whom. Strictly append-only. Never mutated.
- **Interpretation layer**: what the observations mean. Produces findings. Findings have their own lifecycle.

### Ownership model

```
Brand (e.g. "Acme Corp")
  └── Company (e.g. "Acme Deutschland GmbH", or external agency)
        └── Project (cost centre)
              └── Asset assignment
```

Roles: Owner, Manager, Technician. External contractors assigned via separate role with type (Development, DevOps, Design, Consultancy, Security).

An asset with no owner is not an error — it's the default state in a messy organisation. The system surfaces it as a governance finding.

### Lifecycle model

Assets have a lifecycle state: `active`, `under_review`, `deprecated`, `decommissioned`, `abandoned`.

Transitions are events (append-only). An asset doesn't jump from `active` to `decommissioned` — the transition is recorded with who, when, and why.

`abandoned` is typically inferred, not declared: no owner, no recent observations showing activity, possibly still online.

### Confidence and trustworthiness

Not all observations are equally trustworthy. A DNS check that runs hourly is more reliable than a one-off WHOIS lookup. An HTTP probe from inside the network is more authoritative than one from an external scanner.

Observations carry a confidence level. Findings derived from low-confidence observations are flagged accordingly.

---

## 4. Question taxonomy

Balebos is designed around questions, not features. The system should be able to answer:

### Security
- Which public assets are currently vulnerable?
- Which vulnerabilities matter vs scanner noise?
- Which systems expose outdated frameworks or insecure headers?
- Which repos introduced new risk after recent commits?
- What changed in security posture since the last deployment?

### Compliance
- Which assets have no owner?
- Which assets may still process personal data but appear abandoned?
- Which certificates were expired, for how long, and who was responsible?
- Can we prove that an asset was monitored over time?
- Can we show historical evidence for audits?

### Financial
- Which domains or services are still being paid for but appear unused?
- Which assets consume budget despite low value or zero traffic?
- Which zombie systems exist only because nobody disabled them?
- Which domains are auto-renewing without business justification?

### Lifecycle
- Which repos are stale?
- Which frameworks are outdated or end-of-life?
- Which campaigns should have been retired but are still online?
- Which systems depend on unsupported software?

### Operational
- What failed today?
- What degraded compared to yesterday?
- Is the issue transient, external, or structural?
- Which checks are trustworthy and which are noisy?

### Governance
- Who owns this asset?
- Which agency or team is responsible?
- Which brand/country/business unit does it belong to?
- Which findings require escalation to management?

### Change detection
- What appeared in the estate since last week?
- What disappeared?
- What shifted (tech stack change, DNS change, TLS issuer change)?

### Trustworthiness of evidence
- How many times has this asset been observed?
- When was the last successful check?
- Are the scanners themselves healthy?
- How old is the ownership data?

---

## 5. Scanning strategy

### Broad shallow checks (run frequently)
- DNS resolution (A, AAAA, CNAME, MX, NS, TXT) — hourly to daily
- HTTP probing (status, response time, redirects, headers) — hourly to daily
- TLS certificate expiry and issuer — daily
- Port scan (top ports) — weekly

### Deep checks (run less frequently)
- Technology fingerprinting (frameworks, versions) — weekly
- WHOIS / RDAP registration data — weekly
- Dependency vulnerability scanning — daily to weekly
- Security header analysis — weekly
- Accessibility / ARIA checks — monthly

### Event-driven checks
- Post-commit: scan repos for secrets, dependency changes, risky patterns
- Post-deploy: re-probe the affected asset immediately
- Finding-triggered: shallow check finds something suspicious → trigger deeper scan

### Handling heterogeneous tools

Scanners are not all Rust. Some are Go binaries (subfinder, dnsx, httpx), some are Node scripts, some are API calls, some are Playwright runners. The system handles this via a `WorkerManifest` pattern:

```
name: "httpx"
binary: "httpx"
input: stdin (newline-delimited hostnames)
output: stdout (newline-delimited JSON)
observation_type: "http_check"
schedule: "0 */4 * * *"
timeout: 300s
retry: 2
```

The manifest describes what the worker does, not how it's implemented. Balebos manages lifecycle, scheduling, and output parsing. The worker is a black box.

---

## 6. AI escalation strategy

### When an agent should be invoked
- A finding has ambiguous severity and needs contextual reasoning
- A website exhibits suspicious behaviour after a shallow scan (unexpected redirects, content changes)
- A vulnerability scan produces noisy results that need triage
- An asset appears abandoned but might still be meaningful
- An accessibility result needs browser-based validation
- A repo shows risky commit patterns that need deeper inspection

### When an agent must NOT be invoked
- Routine checks with clear pass/fail outcomes
- Certificate expiry (deterministic, no judgement needed)
- DNS resolution (factual, no interpretation needed)
- Any situation where the cost of AI exceeds the value of the answer

### Tooling available to agents
- Browser automation (Playwright)
- WHOIS/RDAP lookup
- Vulnerability databases (CVE, NVD)
- HTTP probing with custom headers/payloads
- Repository analysis (commit history, dependency tree)
- Content analysis (is this page meaningful or a placeholder?)

### Agent output
- Typed observation with `source: "ai-agent"`
- Confidence level
- Reasoning chain (why the agent reached this conclusion)
- Cost metadata (tokens used, API calls made, wall time)
- `requires_review: true` flag — always

### Cost control
- Budget per investigation (e.g. max $2 per agent invocation)
- Daily and weekly spend caps
- Priority queue: high-severity triggers before low-severity
- Circuit breaker: stop invoking agents if error rate exceeds threshold

### Philosophy
AI may reduce entropy before action, but execution and enforcement must remain deterministic, auditable, and testable. An agent's conclusion is an opinion, not a fact. Humans decide, the system records.

---

## 7. Runtime vs orchestration vs domain — where complexity accumulates

### Runtime concerns (scanner execution)
- Running subprocesses, capturing output
- Timeouts, retries, concurrency limits
- Rate limiting for external APIs
- Worker lifecycle (start, health check, kill)
- Failure isolation (one broken scanner doesn't poison others)

**Complexity trajectory**: moderate and stable. The patterns are well-understood. Grows linearly with worker count, not exponentially.

### Orchestration concerns (scheduling and coordination)
- When to run what, on what schedule
- Dependencies between checks (DNS before HTTP)
- Event-driven triggers
- Health monitoring of the scheduling system itself

**Complexity trajectory**: low initially (cron-like), grows if we add dependency graphs or distributed execution. Keep it simple: a tick loop with a schedule table. No workflow engines, no DAGs.

### Domain concerns (what things mean)
- Asset identity, typing, metadata
- Ownership and organisational mapping
- Append-only observation history
- Finding lifecycle and triage
- Risk scoring and signal categorisation
- Compliance evidence and auditability
- Cost/value interpretation

**Complexity trajectory**: this is where most long-term complexity accumulates. The domain model is what the system *is*. Scanners and schedulers are replaceable infrastructure. The domain model is the intellectual property.

### Implication
Do not let orchestration dominate the design. Invest time in the domain model. The scheduler can be a 200-line tick loop. The finding lifecycle, the risk scoring model, the ownership graph — those are the things that will be hard to change later.

---

## 8. Architecture options

### Option A — Conservative monolith

Single Rust binary. CLI + TUI. JSONL storage with optional DataFusion/Parquet for analytics. Scheduled checks via a built-in tick loop. No API, no web UI.

**Strengths**: simplest to build, deploy, and maintain. One binary, one process, one data directory. A single engineer can own the whole thing.

**Weaknesses**: no web dashboard, no API for integrations, limited to what the CLI/TUI can expose. Non-technical stakeholders can't use it directly.

**Team implications**: 1 engineer. Minimal ops. No infrastructure beyond a Linux box and a cron job.

**Risk of overengineering**: low.

### Option B — Monolith with API (recommended)

Single Rust binary with embedded HTTP API. CLI, TUI, and API all served from the same process. DataFusion/Parquet storage. Built-in scheduler. Web dashboard as a separate static frontend consuming the API.

**Strengths**: single deployment unit, but accessible to non-technical stakeholders via API/dashboard. The API is a thin layer over existing domain logic. The dashboard is a separate concern (could be a simple static site).

**Weaknesses**: the binary grows larger. Need to manage HTTP server lifecycle alongside CLI/TUI. The API surface needs design discipline.

**Team implications**: 1-2 engineers. The API is mostly serialisation of existing data. The dashboard can be built by anyone who knows HTML/JS.

**Risk of overengineering**: moderate. The temptation to over-design the API before the domain model is stable. Mitigate by starting read-only.

### Option C — Modular system with Go workers

Rust core for domain model, storage, API, and interpretation. Go workers for scanner execution (leveraging Go's subprocess management, concurrency, and the existing Go-based tool ecosystem). Communication via stdin/stdout or a message queue.

**Strengths**: plays to each language's strengths. Rust for correctness and domain logic. Go for operational tooling and subprocess wrangling. Clean separation of concerns.

**Weaknesses**: two languages, two build systems, two deployment pipelines. Higher onboarding cost. More moving parts. The boundary between Rust and Go needs careful design.

**Team implications**: 2+ engineers, ideally comfortable in both languages. Or one Rust person and one Go person, with clear interface contracts.

**Risk of overengineering**: high if done too early. The Go worker layer only earns its keep when there are 10+ heterogeneous scanners with complex lifecycle requirements. For 4-5 scanners, the current subprocess pattern in Rust is fine.

---

## 9. Recommended architecture

**Option B — Monolith with API.**

A single Rust binary that contains:
- Domain model (assets, observations, findings, org)
- Storage layer (JSONL + DataFusion/Parquet, hexagonal via `StorageEngine` trait)
- Check registry (interpretation, risk scoring, drift detection)
- Scanner execution (subprocess management with manifests)
- Scheduler (in-process tick loop)
- CLI interface
- TUI interface
- HTTP API (read-heavy, eventually read-write)

The web dashboard is a separate project: a static frontend that consumes the API. It can be built later, by different people, in whatever frontend technology makes sense.

**What is deferred:**
- Go workers (use subprocess execution from Rust until scanner count justifies a separate runtime)
- External event bus (use in-process channels)
- AI escalation (build after the finding lifecycle is solid)
- Web dashboard (build after the API is stable)

**What is intentionally boring:**
- Storage: append-only Parquet files, no complex schema migrations
- Scheduling: a tick loop, not a workflow engine
- Configuration: TOML files, not a database of settings
- Deployment: a single binary, not containers-in-Kubernetes

**What is strategically important:**
- The domain model: asset types, lifecycle states, finding lifecycle, ownership graph
- The observation→finding interpretation pipeline: this is the core value of the system
- The append-only history: audit trail, compliance evidence, drift detection all depend on it
- Pure Rust build chain: no C dependencies, compiles anywhere

---

## 10. Data and storage approach

### Inventory store
Assets and their metadata. Mutable (lifecycle transitions), but changes recorded as events.

### Observation store
Append-only log of all scanner output. Parquet files partitioned by time window (e.g. daily). DataFusion queries across partitions transparently.

### Finding store
Findings and their lifecycle events. Append-only (a finding resolution is a new event, not a field update).

### Organisational data
Brands, companies, projects, persons, assignments. Low-volume, low-change. Can live in the same event log or a separate small store.

### Event log
Unified append-only log of all system events: observations, findings, lifecycle transitions, assignments, agent invocations. The primary source of truth. Everything else is a derived view.

### Reporting layer
Materialized views or pre-computed summaries for dashboard-speed queries. Built lazily: compute on first query, cache until invalidated by new data. Not needed until the dataset grows beyond what DataFusion handles in real-time.

### Append-only vs mutable summaries
The event log is append-only. "Current state" views (latest observation per asset, open findings count) are computed at query time or cached as materialized views. This avoids write conflicts, simplifies concurrency, and preserves full audit trail. The cost is that some queries scan more data — but at Balebos scale (thousands of assets, not millions), this is fine.

---

## 11. Governance and operating model

### Who owns data quality
The platform team owns the system. Asset owners own their asset metadata. Nobody owns it all — and that's the point. The system surfaces gaps rather than pretending they don't exist.

### Who reviews escalations
Findings with severity >= High go to the security team. Compliance findings go to the DPO. Financial findings go to controlling. Governance findings (no owner, abandoned asset) go to management. The routing is configurable per finding category.

### Who can override findings
Anyone with an appropriate role can acknowledge or resolve a finding. The override is recorded as an event with who, when, and why. Overrides don't delete the finding — they change its lifecycle state.

### How ownership gaps are surfaced
Assets without an owner produce a standing governance finding. These findings appear in every report, every dashboard view, every API query. They don't go away until someone claims ownership or the asset is decommissioned.

### How the system avoids becoming another stale dashboard
- Findings have lifecycle states. Open findings demand action, not just display.
- Scheduled reports land in inboxes, not on a dashboard nobody visits.
- Exit codes in CLI mode enable integration with alerting systems (PagerDuty, Slack, email).
- The system tracks its own health: scanner uptime, observation freshness, finding age. If the system itself is unhealthy, it says so.

---

## 12. Failure modes and risk analysis

### Scanner failure
A scanner crashes or hangs. **Mitigation**: timeout per worker, automatic retry with backoff, alert if a scanner fails repeatedly. Observations from failed scans are simply absent — the system treats absence of data as a signal ("no recent HTTP check" is itself a finding).

### False positives
A check flags something that isn't actually a problem. **Mitigation**: confidence levels on findings, ability to acknowledge/dismiss, tracking false positive rate per check type over time. High false-positive checks get reviewed or disabled.

### False negatives
A real problem isn't detected. **Mitigation**: broad coverage with many shallow checks is better than deep but narrow coverage. Overlapping checks from different angles. Periodic manual audits to validate the system's coverage.

### Tool drift
A scanner tool updates its output format and Balebos fails to parse it. **Mitigation**: defensive parsing (skip unparseable lines, log warnings, don't crash). Version pinning of external tools in Docker. Parser tests against known output samples.

### Stale ownership data
People leave, teams reorganise, agencies change. The ownership graph becomes outdated. **Mitigation**: periodic ownership review cycle (the system prompts owners to confirm their assets quarterly). Stale ownership produces a finding.

### AI hallucination
An AI agent produces a confident-sounding conclusion that's wrong. **Mitigation**: all AI output requires human review. Agent conclusions are findings with low-to-medium confidence, never high. The reasoning chain is stored and auditable.

### Schedule overload
Too many checks scheduled, system can't keep up. **Mitigation**: priority queues (critical assets first), backpressure (skip low-priority checks when overloaded), monitoring of queue depth and check latency.

### Noisy signals
The system produces so many findings that nobody reads them. **Mitigation**: severity-based filtering in all interfaces, noise tracking per check type, automatic suppression of recurring low-severity findings after acknowledgement.

### Political resistance
Asset owners don't want visibility into their neglected systems. Agencies don't want scrutiny. **Mitigation**: this is an organisational problem, not a technical one. The system can only surface the truth — enforcement is a management decision. Making the data undeniable (historical evidence, trends, comparisons) helps.

---

## 13. Open questions

Things that must be clarified before or during implementation:

- **Asset type granularity**: how many types do we need from day one? Start with domain, subdomain, repository, API — or go broader?
- **Finding severity model**: 3 levels (low/medium/high) or 5 (info/low/medium/high/critical)? More levels = more precision but harder to triage.
- **Ownership enforcement**: should the system *require* an owner for every asset, or just *surface* the gap? Requiring ownership blocks adoption in messy orgs. Surfacing the gap is gentler but less effective.
- **Scanner manifest format**: TOML, JSON, or YAML? How much should be configurable vs convention?
- **DataFusion Parquet partitioning**: by day? by week? by asset type? Affects query performance and file management.
- **API authentication**: internal tool, trust the network? Or proper auth from day one? Depends on deployment model.
- **Dashboard technology**: separate SPA? Server-rendered HTML from the Rust binary? Static site generator? Depends on team skills and maintenance appetite.
- **Multi-instance**: should two Balebos instances be able to share data? Or is single-instance the only supported model? Multi-instance adds significant complexity for marginal benefit at expected scale.

### Dangerous assumptions
- "Everyone will fill in ownership data." They won't. Design for sparse, incomplete data.
- "Scanners will always work." They won't. Design for partial, intermittent observations.
- "Stakeholders will read dashboards." They won't. Design for push-based alerts and scheduled reports.
- "The domain model is stable." It isn't. Design for evolution — append-only events, not rigid schemas.
- "We need all of this now." We don't. Build the domain model first, everything else follows.

### Pilot validation
Before full build-out, validate with a pilot:
- 50-100 real assets from one brand/country
- 3-4 scanner types (DNS, HTTP, TLS, RDAP)
- Basic findings (expired cert, no owner, stale domain)
- One non-technical stakeholder consuming a report
- Measure: does it surface problems nobody knew about? Does anyone act on the findings?
