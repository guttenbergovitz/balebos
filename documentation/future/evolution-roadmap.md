# Balebos — Evolution Roadmap

Status: living document, 2026-04-12

This roadmap describes the concrete steps to evolve Balebos from its current state (a CLI-driven domain/subdomain scanner with organisational context) into a full control and observability system for digital assets.

Each phase has a clear objective, scope, non-goals, risks, and success criteria. Phases are sequential — each builds on the previous — but individual work items within a phase can be parallelised.

For the gap analysis behind this roadmap, see [gap-analysis.md](./gap-analysis.md).
For storage-specific concerns, see [duckdb-concerns.md](./duckdb-concerns.md).

---

## Phase 0 — Domain model hardening

**Objective:** make the domain model expressive enough to represent the full asset landscape, without changing infrastructure or adding new components.

**Scope:**
- Expand `ItemType` to support additional asset categories (API, repository, cloud resource, SaaS dependency, certificate, campaign, hosting environment). Start with 3-4 most useful ones, not all 13 at once.
- Introduce an `AssetLifecycle` enum (`active`, `under_review`, `deprecated`, `decommissioned`, `abandoned`) and store lifecycle transitions as append-only events.
- Design the `Finding` entity: ID, severity, confidence, category (security / compliance / financial / lifecycle / operational), affected asset, linked observations, created_at. Findings are persisted, not ephemeral.
- Define the `ObservationEnvelope` pattern — a generic wrapper with common metadata (target, checked_at, source, confidence) and a typed payload. This replaces the current approach of N separate top-level structs in `reader.rs`.

**Non-goals:**
- New scanners or check types.
- Scheduler or orchestration.
- UI changes.

**Risks:**
- Expanding `ItemType` touches many consumers (TUI, CLI, check, report, diff). Needs careful incremental rollout.
- Over-designing the envelope pattern. Keep it simple — discriminated union, not a trait hierarchy.

**Success criteria:**
- Can register an API endpoint as an asset with lifecycle state.
- Can persist a Finding and query it later.
- Existing domain/subdomain workflow unchanged.
- All existing tests pass.

---

## Phase 1 — Storage evolution

**Objective:** move from blob storage to queryable, indexed, cross-platform storage.

**Scope:**
- Feature-gate DuckDB behind `--features duckdb`. Default build produces a portable binary with JSONL only.
- Implement `DataFusionStorage` adapter: write events as Parquet files, query with SQL via DataFusion. See [duckdb-concerns.md](./duckdb-concerns.md) for rationale.
- Add temporal query support: "show observations for asset X between date A and date B".
- Add basic indexing: filter by `item_type`, `target`, `checked_at` without full table scan.
- Migration tooling: JSONL → Parquet converter.

**Non-goals:**
- Materialized views or pre-computed summaries (premature before we know the query patterns).
- Dropping DuckDB support (keep behind feature flag for users who want it).

**Risks:**
- DataFusion is a large dependency. Compile times will increase. Mitigate with feature gating.
- Parquet file lifecycle management (rotation, compaction) adds operational complexity. Start simple: one file per scan batch.

**Success criteria:**
- `cargo build` without features compiles in under 30 seconds on M-series Mac.
- `cargo build --features datafusion` produces a binary that stores and queries events via Parquet.
- Cross-compilation to `x86_64-unknown-linux-musl` works without C toolchain.
- Temporal queries run in under 1 second on 500K observations.

---

## Phase 2 — Interpretation layer

**Objective:** replace the current `check.rs` approach with a proper findings engine.

**Scope:**
- Implement the `Finding` lifecycle: create → acknowledge → assign → resolve → reopen. Each transition is an append-only event.
- Build a check registry: each check is a named function that takes observations and produces findings. Checks are registered, not hardcoded in a single function.
- Add severity classification beyond Warning/Critical: Info, Low, Medium, High, Critical.
- Add basic scoring: per-asset risk score as a weighted sum of active findings.
- Persist findings to storage. Findings are queryable ("show all unresolved critical findings").

**Non-goals:**
- AI-driven analysis.
- Complex scoring algorithms (simple weighted sum is enough to start).
- Drift detection across observation history (Phase 3).

**Risks:**
- Over-engineering the check registry. It should be a `Vec<Box<dyn Check>>`, not a plugin system.
- Finding fatigue: too many findings with no triage path. Mitigate by starting with few, high-signal checks.

**Success criteria:**
- `balebos check` persists findings and reports them on subsequent runs.
- `balebos findings` lists open findings with severity and affected asset.
- A finding can be acknowledged via CLI (`balebos finding ack <id>`).
- The report includes finding counts by severity.

---

## Phase 3 — Worker abstraction and scheduling

**Objective:** decouple check execution from the Balebos binary. Enable heterogeneous, scheduled scanning.

**Scope:**
- Define a `WorkerManifest` spec: name, binary path (or API endpoint), input format, output format, expected observation type, default schedule, timeout, retry policy.
- Extract existing probes (subfinder, dnsx, httpx, rdap) into manifest-described workers.
- Build a minimal scheduler: cron-like execution of workers based on manifest schedules. Initially in-process (a background thread with a tick loop). Not an external scheduler.
- Add timeout enforcement and failure isolation per worker.
- Add retry logic with backoff.

**Non-goals:**
- Distributed execution. Single-machine, single-process.
- Plugin hot-loading. Workers are external binaries described by static manifests.
- Event-driven triggers (Phase 4).

**Risks:**
- Manifest format design. Keep it minimal: TOML or JSON, not a DSL.
- Scheduler complexity. A `loop { sleep(tick); check_schedules(); }` is the right starting point.

**Success criteria:**
- Existing probes work via manifests without code changes to the probe logic.
- A new check type (e.g., security headers via a shell script) can be added by writing a manifest and a script, without touching Rust code.
- Scheduled checks run unattended and write observations to storage.
- A hanging worker is killed after its timeout without affecting other workers.

---

## Phase 4 — API, eventing, and drift detection

**Objective:** make Balebos's data accessible to non-CLI consumers. Add change-over-time analysis.

**Scope:**
- HTTP API: read-only initially. Endpoints for inventory, observations, findings, org data. JSON responses.
- Event hooks: after a scan completes or a finding is created, emit an event. Initially just in-process channels (`tokio::sync::broadcast`). Enables reactive checks: "when HTTP check finds new tech, trigger a deeper scan".
- Historical drift detection: compare current observations with previous ones for the same asset. Detect: response time regression, tech stack change, TLS issuer change, DNS record change, status code change.
- API-triggered scans: POST to trigger a check for a specific asset.

**Non-goals:**
- Web dashboard (separate project, consumes the API).
- External message bus (NATS, Redis). In-process channels first.
- Authentication/authorisation on the API (internal tool, trust the network initially).

**Risks:**
- API design lock-in. Start with a minimal, read-heavy API. Don't expose mutation until the domain model is stable.
- Drift detection can be noisy. Require minimum observation count before flagging drift.

**Success criteria:**
- `curl localhost:8080/api/assets` returns inventory as JSON.
- `curl localhost:8080/api/findings?severity=critical` returns filtered findings.
- A tech stack change between two scans produces a finding automatically.
- An event-driven deeper check triggers when a shallow check finds something suspicious.

---

## Phase 5 — AI escalation

**Objective:** add selective, tool-using AI agents for ambiguous or high-value cases.

**Scope:**
- Define trigger conditions: which findings or observation patterns invoke an agent.
- Agent tooling: what external tools/APIs the agent can call (browser, WHOIS, vulnerability databases, etc.).
- Agent output: a typed observation with `source: "ai-agent"`, confidence level, reasoning chain, and cost metadata.
- Cost control: budget per investigation, daily/weekly cap.
- Human review gate: agent conclusions are findings with `requires_review: true`. No automated enforcement from AI output.
- Audit trail: full log of agent invocations, tool calls, inputs, outputs.

**Non-goals:**
- AI as the primary scanning layer.
- Automated remediation based on AI conclusions.
- Training custom models.

**Risks:**
- AI hallucination producing false findings. Mitigate with confidence scoring and mandatory human review.
- Cost overrun. Hard caps per investigation and per time window.
- Scope creep: "let the AI do everything". The philosophy is clear: AI reduces entropy before human decision, it doesn't replace the decision.

**Success criteria:**
- An agent is triggered by a specific finding pattern (e.g., "suspicious redirect chain").
- The agent investigates using defined tools and writes a structured conclusion.
- The conclusion appears as a finding requiring human review.
- Total AI spend is tracked and respects configured budgets.
- Agent invocations are fully auditable.

---

## Cross-cutting concerns (all phases)

**Testing.** Each phase adds tests for its new functionality. Domain model changes get unit tests. Storage adapters get integration tests. The check registry gets property-based tests where practical.

**Documentation.** Each phase updates `architecture.md` and relevant specs. New ADRs for significant decisions.

**Backwards compatibility.** JSONL format remains stable. New record types are additive. Old records are always parseable.

**Single binary.** Balebos remains a single binary with feature flags for optional backends. No microservices until the data proves a single process can't cope (unlikely at the expected scale).
