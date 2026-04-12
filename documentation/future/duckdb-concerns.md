# DuckDB — Platform Dependency and Pure Rust Alternatives

Status: open question, 2026-04-12

## The problem

DuckDB is a strong analytical database — fast, embeddable, zero-config. For Balebos's append-only event log with analytical queries, it's a good semantic fit.

The concern is the **native C/C++ dependency chain**.

The `duckdb` Rust crate (v1.4.2) wraps `libduckdb`, a substantial C++ library. This means:

- **Build-time**: needs C/C++ toolchain. The `bundled` feature compiles DuckDB from source, adding 5-10 minutes to builds.
- **Cross-compilation**: building for a different target requires the correct native library. Docker helps but doesn't eliminate friction.
- **Musl / Alpine**: static linking with musl for minimal containers is problematic. DuckDB doesn't officially support musl targets.
- **ARM**: DuckDB supports aarch64, but pre-built binaries lag behind x86. Relevant for M-series Macs and ARM cloud instances.
- **Binary size**: the DuckDB library adds significant weight to the final binary.
- **Upgrade risk**: DuckDB crate updates can break compatibility with the native library or change file format.

For a system that should survive years and deploy anywhere with minimal friction, a C++ dependency on the critical path is worth questioning.

---

## Pure Rust alternatives

### Option A — Apache DataFusion + Parquet

**What it is.** DataFusion is a pure Rust query engine built on Apache Arrow. It provides full SQL support, reads/writes Parquet, JSON, CSV natively. It's the query engine behind InfluxDB, GreptimeDB, and dozens of production systems.

**Why it fits.**
- Pure Rust, zero C dependencies. Compiles to any target Rust supports.
- Native SQL support — no need to hand-roll query logic.
- Parquet as the storage format gives columnar analytics with compression, and files are human-inspectable with standard tools.
- Arrow in-memory format means efficient analytical processing.
- Extensible: custom table providers, UDFs, optimizer rules.
- Apache 2.0 licence, massive community (53+ releases on crates.io, 18M+ downloads).

**How it would work in Balebos.**
- Events written as append-only Parquet files (one file per time window or batch).
- DataFusion registers these files as tables and runs SQL queries over them.
- Temporal queries ("TLS history for example.com since March") become straightforward SQL with predicate pushdown.
- The `StorageEngine` trait gets a new `DataFusionStorage` adapter.
- JSONL remains for interchange and human-readable backup.

**Trade-offs.**
- DataFusion is a query engine, not a database. There's no built-in WAL or transaction semantics. For Balebos's append-only model, this is fine — we don't need transactions. But it means managing file lifecycle (compaction, rotation) ourselves.
- The crate is large (~22K SLoC) and has many transitive dependencies (Arrow ecosystem). Compile time is meaningful, though still less than DuckDB-from-source.
- Parquet files are not human-readable (unlike JSONL). But we keep JSONL as the interchange format anyway.

**Verdict: the strongest candidate.** SQL, columnar analytics, pure Rust, battle-tested in production. The append-only Parquet model maps directly to Balebos's event-sourced architecture.

### Option B — Fjall (LSM-tree key-value store)

**What it is.** Fjall is a pure Rust LSM-tree storage engine, architecturally similar to RocksDB. V3 (January 2026) is mature, well-benchmarked, and actively maintained. ~42K LoC, compiles in ~3.5s, 2.2 MB binary overhead.

**Why it might fit.**
- Pure Rust, zero C dependencies.
- ACID transactions, crash-safe, MVCC.
- Excellent write performance (LSM-tree is optimised for append-heavy workloads).
- Compression support (LZ4, planned Zstd).
- Very small binary footprint.

**Why it probably doesn't fit.**
- Key-value store, not a query engine. No SQL, no columnar analytics. Every "query" would be a manual key scan with application-level filtering.
- Balebos needs analytical queries: "count assets by status", "show TLS history over time", "find all assets without an owner". Implementing these on top of a KV store means reinventing query planning.
- No JSON extraction, no aggregations, no joins.

**Verdict: good for different problems.** If Balebos were a high-throughput event ingestion system with simple key lookups, Fjall would be excellent. For analytical queries over heterogeneous event data, it's the wrong tool.

### Option C — redb (B-tree key-value store)

**What it is.** Pure Rust embedded key-value database, inspired by LMDB. Stable, maintained, 4.1K GitHub stars.

**Same limitation as Fjall**: key-value only, no query engine. Even less suitable than Fjall for analytical workloads (B-tree is worse than LSM for append-heavy patterns).

**Verdict: not a fit** for Balebos's query needs.

### Option D — Limbo (pure Rust SQLite rewrite)

**What it is.** Turso's pure Rust rewrite of SQLite, compatible at the file format and SQL level. Async I/O, WASM support, DST testing. Still experimental but progressing rapidly.

**Why it's interesting.**
- Pure Rust, SQLite-compatible SQL.
- If it stabilises, it gives SQLite's query capabilities without the C dependency.
- File format compatibility means existing SQLite tooling works.

**Why it's risky today.**
- Still experimental. The project description says "an ambitious experiment". Not production-ready for a system that should survive years.
- Feature coverage is incomplete compared to SQLite proper.
- If it fails or stalls, we're stuck on an abandoned dependency.

**Verdict: watch, don't adopt yet.** Revisit in 6-12 months. If Limbo reaches stability, it could be a compelling middle ground between DataFusion (complex, analytical) and raw KV stores.

### Option E — Hand-rolled on top of serde + Parquet (no query engine)

**What it is.** Skip the query engine entirely. Read Parquet/JSONL files, deserialise into Rust structs, filter and aggregate in application code.

**Why it's tempting.** Zero dependencies beyond serde and the parquet crate. Full control. No query planning overhead.

**Why it doesn't scale.** Works fine for 1,000 assets with 10,000 observations. Falls apart when you need temporal queries, aggregations, joins between inventory and observations, or ad-hoc analysis. Every new question becomes a new Rust function. This is what `read_all_records()` in the current JSONL reader already is — and it already doesn't scale.

**Verdict: where we are today.** Fine for now, not where we want to be.

---

## Recommendation

**DataFusion + Parquet** is the clear winner for Balebos:

1. Pure Rust — compiles anywhere, no C toolchain, no platform-specific library management.
2. SQL — temporal queries, aggregations, joins become declarative instead of hand-rolled.
3. Parquet — columnar, compressed, append-friendly. One file per scan batch or time window.
4. The `StorageEngine` trait already abstracts the backend — adding a `DataFusionStorage` adapter is a clean change.
5. JSONL stays as the interchange and backup format. Existing data migration is straightforward (JSONL → Parquet conversion).

### Migration path

1. Feature-gate `duckdb` behind a Cargo feature flag (immediate, low effort).
2. Add a `DataFusionStorage` adapter behind a `datafusion` feature flag.
3. Run both in parallel for a release cycle to validate correctness.
4. Make DataFusion the default. Keep JSONL as fallback.
5. Drop the DuckDB adapter when confident.

### Action items

- [ ] Feature-gate the `duckdb` dependency behind `--features duckdb`
- [ ] Ensure `cargo build` without features produces a clean, portable binary (JSONL-only)
- [ ] Prototype `DataFusionStorage` adapter: write events as Parquet, query with SQL
- [ ] Benchmark DataFusion vs DuckDB on realistic dataset (10K assets, 500K observations)
- [ ] Test cross-compilation to `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-gnu`
