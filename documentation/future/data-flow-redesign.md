# Balebos — Data Flow Redesign

Status: planned, 2026-04-12

This document describes how data moves through Balebos after migrating from DuckDB to DataFusion + Parquet, and why this makes adding new scanners significantly easier.

---

## Current flow (DuckDB / JSONL)

```
Scanner (e.g. httpx)
  → stdout: JSON lines
  → Runner: reads stdout, manages subprocess lifecycle
  → Parser: raw JSON → typed Rust struct (HttpObservation, DnsObservation, etc.)
  → Writer: struct → serialise to JSON string → INSERT INTO events (data) VALUES (json::JSON)
  → Reader: SELECT data FROM events → deserialise every row → big match on item_type → sort into Records struct
  → Consumers: check.rs, report.rs, diff.rs, TUI — all receive the fully-loaded Records struct
```

### Problems with this flow

1. **DuckDB stores everything as a JSON blob** in a single `events` table with columns `(id, data)`. No indexes on content. Every query loads and parses all rows.

2. **`reader.rs` has a massive match statement** that grows with every new `ItemType`. Adding a new observation type means touching `ItemType` enum, `reader.rs` parser, `Records` struct, and every consumer.

3. **`read_all_records()` loads everything into memory.** At 500K observations this starts to hurt. No way to query "just HTTP checks for example.com from last week" without loading the entire event log.

4. **DuckDB requires C/C++ toolchain.** Cross-compilation friction, slow builds, musl incompatibility.

---

## New flow (DataFusion + Parquet)

```
Scanner (any language — Go, Node, Python, shell)
  → stdout: JSON lines
  → Runner: reads stdout, manages subprocess (timeout, retry, kill)
  → Parser: raw JSON → typed Rust struct (HttpObservation, DnsObservation, or any new type)
  → Writer: struct → append to Parquet file (partitioned by day)
         → also write JSONL copy for backup/interchange
  → DataFusion: registers Parquet directories as SQL tables
  → Query: SQL over Parquet files with automatic partition pruning
  → Checks: query only the data they need via SQL
  → Consumers: CLI, TUI, API — all query through DataFusion
```

### Step by step

**1. Scanner produces data.**
Unchanged. Any external tool that writes JSON lines to stdout. httpx, dnsx, subfinder, rdap, or a new custom script. Balebos doesn't care what language the scanner is written in.

**2. Runner manages the subprocess.**
Unchanged. Balebos launches the process, feeds input via stdin, reads stdout line by line, enforces timeout, retries on failure. Exactly as today.

**3. Parser converts to typed structs.**
Unchanged. Each JSON line is deserialised into a typed Rust struct (`HttpObservation`, `DnsObservation`, etc.) with validation and normalisation.

**4. Writer appends to Parquet.**
This is the key change. Instead of serialising to a JSON string and inserting into a DuckDB blob column:
- The typed struct is written to a Parquet file.
- Files are partitioned by date: `data/observations/2026-04-12.parquet`, `data/observations/2026-04-13.parquet`, etc.
- Different record categories can go to different directories: `data/observations/`, `data/findings/`, `data/inventory/`.
- A JSONL copy is written in parallel for backup, interchange, and human readability.
- Writing a new struct type requires only that it implements `Serialize` — no new branches in storage code.

**5. DataFusion registers tables.**
On startup, Balebos tells DataFusion: "the directory `data/observations/` is a table called `observations`". DataFusion discovers all Parquet files in the directory and treats them as partitions of one logical table.

**6. Queries use SQL.**
Instead of `read_all_records()` loading everything into a `Records` struct, consumers query only what they need:

```sql
-- Latest HTTP check for a specific domain
SELECT * FROM observations
WHERE item_type = 'http_check' AND target = 'example.com'
ORDER BY checked_at DESC LIMIT 1

-- All TLS observations in the last 6 months (for audit evidence)
SELECT * FROM observations
WHERE item_type = 'http_check' AND tls IS NOT NULL
AND checked_at > '2025-10-12'

-- Count of assets by lifecycle state
SELECT lifecycle_state, COUNT(*) FROM inventory
GROUP BY lifecycle_state

-- Open critical findings
SELECT * FROM findings
WHERE severity = 'critical' AND resolved_at IS NULL
```

DataFusion automatically prunes partitions: a query for April data doesn't touch January files.

**7. Checks query through SQL.**
Instead of `check.rs` receiving a pre-loaded `Records` struct and iterating in memory, each check runs a focused SQL query. A TLS expiry check queries only TLS observations from the last 24 hours, not the entire event history.

**8. Findings written back.**
When a check produces a finding, it's written to `data/findings/` as a Parquet record. Findings are queryable the same way as observations.

---

## What changes for adding a new scanner

### Today: 5+ files to touch

1. New struct in `app/probe/types.rs`
2. New variant in `ItemType` enum (`inventory/item.rs`)
3. New parse branch in `reader.rs` (deserialise the JSON, push to the right Vec)
4. New field in `Records` struct (`storage/reader.rs`)
5. New handling in every consumer that uses `Records` (check, report, TUI, etc.)
6. DuckDB still stores it as a JSON blob — no schema benefit

### After: 2 files to touch

1. New struct in `app/probe/types.rs` — define the shape of the observation
2. New scanner manifest (TOML file) — describe how to run the scanner

That's it. The struct implements `Serialize`, the Writer writes it to Parquet, DataFusion reads it via SQL. No parser branches, no `Records` struct expansion, no per-type storage code.

A new scanner manifest looks like:

```toml
[scanner]
name = "security-headers"
binary = "./tools/check-headers.sh"
input = "stdin"
output = "stdout"
format = "json-lines"
observation_type = "security_header_check"

[schedule]
cron = "0 6 * * *"  # daily at 6am

[limits]
timeout = "120s"
retry = 2
```

The scanner itself can be a shell script, a Go binary, a Node script, a Python tool — anything that reads hostnames from stdin and writes JSON observations to stdout.

---

## File layout on disk

```
data/
├── inventory/
│   └── assets.parquet           # asset identity records
├── observations/
│   ├── 2026-04-10.parquet       # all observations from April 10
│   ├── 2026-04-11.parquet
│   ├── 2026-04-12.parquet
│   └── ...
├── findings/
│   ├── 2026-04.parquet          # findings created in April
│   └── ...
├── org/
│   └── org.parquet              # brands, companies, projects, assignments
├── backup/
│   └── inventory.jsonl          # full JSONL backup (human-readable)
└── scanners/
    ├── httpx.toml               # scanner manifests
    ├── dnsx.toml
    ├── subfinder.toml
    └── security-headers.toml
```

Parquet files are append-only. A day's observations go into one file. Old files are never modified. Compaction (merging small files into larger ones) can be added later if needed, but at expected scale (thousands of assets) it's unlikely to matter.

---

## What the StorageEngine trait looks like after

```rust
pub trait StorageEngine: Send + Sync {
    fn name(&self) -> String;

    // Write
    fn append_observation<T: Serialize>(&self, obs: &T) -> Result<()>;
    fn append_batch<T: Serialize>(&self, observations: &[T]) -> Result<()>;

    // Query (returns Arrow RecordBatches or deserialized structs)
    fn query(&self, sql: &str) -> Result<Vec<RecordBatch>>;

    // Legacy compatibility
    fn save_jsonl(&self, json: &str) -> Result<()>;
}
```

The `query()` method replaces `read_all_records()`. Consumers write SQL instead of loading everything and filtering in Rust. The JSONL method stays for backward compatibility and backup.

---

## Migration path

1. **Feature-gate DuckDB** — `cargo build` without features uses JSONL only. No breaking change.
2. **Add Parquet writer** — observations written to both JSONL and Parquet in parallel. DataFusion reads Parquet.
3. **Add DataFusion query layer** — new `StorageEngine` impl. Consumers migrated one by one from `read_all_records()` to SQL queries.
4. **Validate** — run both paths in parallel, compare results. Benchmark on real data.
5. **Default to DataFusion** — make it the default backend. JSONL stays for backup.
6. **Drop DuckDB** — remove the `duckdb` feature once DataFusion is proven.

Each step is independently shippable. No big bang migration.

---

## What this does NOT solve

- **Scheduling**: this is a storage and query redesign, not a scheduler. Scanners still run when you tell them to (CLI or cron). Built-in scheduling is a separate piece of work.
- **Web UI / API**: the query layer enables it (you can serve SQL results as JSON over HTTP) but the API itself is separate work.
- **AI escalation**: entirely separate concern, depends on the findings engine being solid first.
- **Finding lifecycle**: the storage supports it (findings as Parquet records with lifecycle events) but the triage workflow (acknowledge, assign, resolve) is application logic to be built on top.
