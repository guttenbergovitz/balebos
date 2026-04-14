# Balebos - Project Context & Instructions

Balebos is a system control layer designed to keep infrastructure in a known, observable state. It scans, normalizes, and tracks assets (domains, services, repositories) and enforces operational invariants to reduce system entropy.

## Project Overview

- **Purpose:** Infrastructure asset inventory, observability, and risk management.
- **Core Technology:**
  - **Language:** Rust (requires 1.94.1+ for Polars 0.53 compatibility).
  - **Storage:** Apache Parquet (columnar) for performance, JSONL for interchange.
  - **Data Engine:** Polars for high-performance in-memory DataFrame operations.
  - **UI:** Interactive TUI via `ratatui` and a comprehensive CLI.
  - **Orchestration:** Wraps specialized OSINT tools (`subfinder`, `dnsx`, `httpx`, `rdap`).
- **Architecture:**
  - **Hexagonal Storage:** `StorageEngine` trait allows swapping backends (currently `ParquetStorage`).
  - **Event-Sourced:** Append-only records preserve a full audit trail of all observations.
  - **Layered Design:** `src/domain` (models), `src/storage` (persistence), `src/app` (business logic), `src/cli` & `src/tui` (interfaces).

## Building and Running

### Local Development
- **Build:** `make build` (builds release binary)
- **Run TUI:** `make run-ui` or `cargo run -- ui`
- **Scan Domain:** `make scan domain=example.com` or `cargo run -- scan example.com`
- **List Assets:** `make list` or `cargo run -- list`
- **Tests:** `make dev-test`
- **Lint/Format:** `make dev-fix` (runs `rustfmt` and `clippy --fix`)

### Docker (Recommended)
- **Build:** `make docker-build`
- **Run TUI:** `docker compose run --rm balebos ui`
- **Scan Domain:** `docker compose run --rm balebos scan example.com`

## Development Conventions

### Coding Style
- **Idiomatic Rust:** Follow standard Rust conventions (naming, safety, performance).
- **Functional Approach:** Prefer functional patterns (iterators, closures) where appropriate.
- **Error Handling:** Use the `BalebosError` enum in `src/error.rs` via `thiserror`. All fallible operations must return `Result<T>`.
- **Typing:** Strict discipline in Rust; avoid `unwrap()` in production code—prefer proper error propagation or informative `expect()`.

### Testing & Quality
- **TDD:** Prioritize writing tests for new business logic.
- **Validation:** Always validate domain inputs using the logic in `src/domain`.
- **Storage Integrity:** Ensure all new record types are properly handled by the Parquet storage buffer (default flush at 100 items).

### Documentation
- **Technical Specs:** Refer to `documentation/architecture.md` and `documentation/specs/data-model.md`.
- **Comments:** Provide professional English comments explaining the "why" for both humans and LLMs.

## Data Model & Storage
- **Inventory vs. Observations:** Assets (Domains) represent identity; Probes (HTTP/DNS checks) represent temporal observations.
- **Partitioning:** Data is stored in `data/` partitioned by record type (e.g., `domain.parquet`, `finding.parquet`).
- **Schema:** Single-column `raw_json` schema to ensure 100% data integrity for complex, evolving probe observations (e.g., nested arrays/objects from `httpx`). Long-term, a return to granular columnar storage is considered as schema management and Polars stability improve.

## Key Subsystems
- **Findings:** Persisted, queryable issues with severity (Critical to Info) and lifecycle (Open, Acknowledged, Resolved).
- **Org Model:** Hierarchical ownership (Brand -> Company -> Project -> Asset).
- **Probes:** Streaming batch architecture that feeds tools via stdin for native Go-concurrency speeds.
