# Balebos — Gap Analysis and Evolution Plan

Status: living document, last updated 2026-04-12

This document maps what Balebos already does well, what needs extending, and what's missing entirely — measured against the full vision of a control and observability system for digital assets across a disorganised enterprise.

The original design exercise was codenamed "Overseer". Balebos is the actual project.

---

## What we've got and what it's worth

### Solid foundations (keep, extend)

**Identity / observation separation.** The split between `InventoryItem` (what exists) and `*Observation` types (what was seen, when) is exactly the right pattern. Append-only, timestamped, source-tagged. This is the single most important architectural decision and it's already in place.

**Organisational model.** Brand → Company → Project → Asset with typed IDs, role assignments (Owner, Manager, Technician), and external contractor support. This is surprisingly mature for a v1.3 system. Covers ownership, cost allocation, and vendor governance out of the box.

**Hexagonal storage.** `StorageEngine` trait with JSONL and DuckDB adapters, auto-migration between them. Clean boundary. Easy to swap backends later without touching domain logic.

**Scanner integration pattern.** Streaming subprocess execution (stdin/stdout piping to Go tools) with real-time JSON parsing. Pragmatic, performant, and aligned with Unix philosophy. The *pattern* is right even if the current implementation is too tightly coupled.

**Observation types.** HTTP, DNS, ASN, TLS, RDAP, hosting detection — each with proper typing, confidence levels, and error categorisation. The `HttpObservation` XOR invariant (status or error, never both) is a nice touch.

### Partial — started but insufficient

**Asset types.** Currently hardcoded to `domain` and `subdomain` via the `ItemType` enum. The full vision needs 13+ types: API, repository, cloud resource, SaaS dependency, certificate (as standalone entity), campaign/landing page, hosting environment, etc. This is a domain model expansion, not a rewrite — but it touches `ItemType`, `InventoryItem`, storage parsing, and every consumer of inventory data.

**Lifecycle tracking.** Assets have `discovered_at` and staleness checks, but no formal lifecycle state. Need a proper state machine: `active`, `under_review`, `deprecated`, `decommissioned`, `abandoned`. Transitions should be events (append-only), not field updates.

**Interpretation.** `check.rs` runs six hardcoded checks and produces ephemeral `Issue` structs that get printed and discarded. Good as a proof of concept for the *type* of analysis needed. Not good as a foundation for persisted findings, risk scoring, or triage workflows.

**Historical queries.** The append-only event log exists, but DuckDB stores everything as `(id, data::JSON)` with no indexes. Temporal queries ("show TLS history for example.com over 6 months", "prove continuous monitoring for an audit") require a full table scan and in-memory parse. See [DuckDB concerns](./duckdb-concerns.md).

**Change detection.** `diff.rs` computes new/missing domains over a time window. Useful, but doesn't detect *within-asset drift* (response time degradation, tech stack change, security posture regression).

### Missing — to be designed from scratch

**Finding lifecycle.** No concept of a `Finding` as a first-class, persisted entity with ID, severity, confidence, affected asset, observation evidence, created/acknowledged/resolved timestamps, and assignee. Current `Issue` is a formatted string. The entire interpretation layer needs this as its core output type.

**Risk / scoring model.** No aggregation of signals into a risk score per asset. No distinction between security signals, cost signals, lifecycle signals, and compliance signals. The question taxonomy from the original design exercise is entirely unimplemented.

**Scheduler / orchestrator.** Balebos is a CLI tool. The full vision is a continuous system with heterogeneous schedules (DNS hourly, RDAP weekly, vulnerability nightly), event-driven triggers (post-commit, post-deploy), retry logic, timeout budgets, and worker health monitoring. None of this exists.

**Worker abstraction.** Current probes are hardcoded functions in `app/probe/`. Need a plugin-like model where a worker can be a Go binary, Node script, API call, or Playwright runner — described by a manifest, not compiled in.

**AI escalation pipeline.** Zero infrastructure. The pattern should be: surface scan → suspicious signal → agent invocation → agent uses tools → agent writes a typed observation with `source: "ai-agent"` → human reviews. Needs trigger conditions, cost budgets, audit trail, and clear separation between deterministic checks and non-deterministic AI judgement.

**Event bus / pub-sub.** No eventing system. Required for: triggering deeper checks from shallow findings, notifying downstream consumers, driving the scheduler. Decision needed: embedded (in-process async channels) vs external (NATS, Redis Streams). Start embedded, graduate later.

**Web UI / API.** TUI is useful for operators but doesn't serve non-technical stakeholders (compliance, finance, management). Need an HTTP API and a web dashboard. Separate deployment target.

**Governance operating model.** No escalation flow, no review points, no concept of "who signs off on a finding". This is partly a software problem (the system needs to model it) and partly an organisational problem (someone needs to own the process).

---

## Reuse potential by component

| Component | Current state | Reuse | Work needed |
|---|---|---|---|
| Domain model core | Mature | High | Extend `ItemType`, add lifecycle FSM, extensible asset metadata |
| Observation model | Good pattern | High | Generalise into `ObservationEnvelope` with typed payload |
| Org model | Ready | Very high | Minimal changes |
| Storage architecture | Solid | High | Schema evolution, indexes, temporal queries, projections |
| Scanner integration | Works | Medium | Extract to worker abstraction with manifest |
| Interpretation / findings | Embryonic | Low | Redesign: Finding lifecycle, scoring, risk signals |
| Orchestration | Absent | None | New component |
| AI escalation | Absent | None | New component |
| Eventing | Absent | None | New component |
| Web UI / API | Absent | None | New component |

---

## Suggested evolution phases

### Phase 0 — Domain model hardening

Expand `ItemType` beyond domain/subdomain. Introduce lifecycle states as append-only events. Design the `Finding` entity. Define the `ObservationEnvelope` pattern. None of this requires new infrastructure — it's pure Rust domain modelling with the existing storage.

### Phase 1 — Storage maturity

Evolve DuckDB schema from blob storage to properly indexed tables. Add temporal query support. Consider materialized views for dashboard-speed queries. Address the [DuckDB platform dependency concerns](./duckdb-concerns.md).

### Phase 2 — Worker abstraction and scheduling

Extract probe logic into a manifest-driven worker model. Build a scheduler (cron-like initially, event-driven later). Add retry, timeout, and failure isolation per worker.

### Phase 3 — Interpretation and findings

Build the Finding lifecycle. Implement risk scoring. Add historical drift detection. Persist findings as events. Build the triage workflow (acknowledge, assign, resolve, reopen).

### Phase 4 — API and integrations

HTTP API exposing inventory, observations, findings. Webhook support. Export capabilities beyond CLI. This unlocks the web dashboard and external integrations.

### Phase 5 — AI escalation

Trigger conditions based on findings. Agent tooling. Cost control. Audit trail. Human review gates. This is the last phase because it depends on all previous layers being solid.

---

## What not to overdesign early

- Don't build a plugin system before you have 3+ worker types that actually need it.
- Don't add an event bus before in-process channels prove insufficient.
- Don't build a web UI before the API is stable.
- Don't introduce microservices. A single binary with clear module boundaries is the right shape for a small team.
- Don't optimise DuckDB queries before you have enough data to see real bottlenecks.
- Don't design the AI escalation protocol before you've manually triaged 100+ findings and understand which patterns actually benefit from AI.
