# Balebos - Architecture and Concepts

This document describes the architectural decisions, core concepts, and design philosophy behind Balebos.

## System Philosophy

Balebos is built on these core principles:
- **Simplicity over complexity** - Use the simplest solution that works
- **Append-only storage** - Never modify historical data
- **Query-time interpretation** - Logic happens at read time, not write time
- **Operational focus** - Designed for practical infrastructure management
- **Minimal dependencies** - Reliance on standard tools and formats

## Architecture

Balebos is organized into focused components, each with a clear responsibility:

### Core Components

1. **Domain Layer**
   - Represents core business entities (Domain, etc.)
   - Contains validation and entity creation logic
   - Isolated from storage and presentation concerns

2. **Storage Layer**
   - Implements a **Port/Adapter (Hexagonal)** architecture
   - **Port**: `StorageEngine` trait defines the persistence interface
   - **Adapter (JSONL)**: Legacy/interchange backend using append-only text files
   - **Adapter (DuckDB)**: Primary high-performance OLAP backend with SQL support
   - Handles auto-migration from JSONL to DuckDB on first run
   - Provides thread-safe access via `Arc<dyn StorageEngine>`

3. **Application Layer**
   - Contains business logic and use cases
   - Orchestrates domain and storage operations
   - Implements interpretation and analysis

4. **CLI Layer**
   - Handles command-line interface
   - Parses arguments and options
   - Presents results to the user

5. **TUI Layer** (`src/tui/`)
   - Interactive terminal dashboard built with `ratatui` and `crossterm`
   - **`model.rs`** — Loads and aggregates inventory data into `AssetState` groups (root domains with subdomains), computes global statistics, and detects external redirects by comparing root domains (handles www, subfolders, sibling subdomains, and two-part TLDs like `.co.uk`)
   - **`app.rs`** — Application state machine managing navigation, input modes (Normal, Editing, Filtering), and background scan orchestration via `Arc<Mutex<>>` shared state
   - **`ui.rs`** — Rendering layer: header with live stats and scan progress, collapsible asset table, detail inspector (DNS, HTTP, TLS, redirect chains), filter bar, input modal, and keyboard help footer
   - Background scanning runs in a separate thread with progress updates piped back to the UI via shared `scan_status` — no stdout pollution in alternate screen mode

### Data Flow

```
CLI / TUI → Application → Domain → Storage
                  ↑           ↑
                  └──── Storage
```

This organization ensures:
- Clear separation of concerns
- Testability of business logic
- Flexibility to change storage or interface

## Key Concepts

### Inventory

The central registry of all infrastructure assets. Unlike traditional asset databases:
- It preserves all historical records (never updates in place)
- It tolerates duplication and noise
- It separates raw data from interpretation
- It prioritizes completeness over cleanliness

### Validation

Balebos performs structural validation on domains before they are added to the inventory:
- **Non-empty** - The domain cannot be an empty string after normalization.
- **Top-Level Domain (TLD)** - Must contain at least one dot separating labels.
- **Label Structure** - Labels must be alphanumeric (plus hyphens) and cannot start or end with a hyphen.
- **TLD Length** - The TLD part must be at least 2 characters long.

Validation is purely structural and does not require active DNS lookups.

### Normalization

Balebos stores both raw and normalized data:
- Domain names are converted to lowercase
- Protocol prefixes (http://, https://) are removed
- www prefixes are removed
- Trailing slashes are removed

Normalization is stored alongside raw input, not instead of it.
Interpretation still happens at query time, using the normalized form while
preserving the original data for reference and auditing.

### Hybrid Persistence

Balebos combines the simplicity of append-only text logs with the power of an analytical database:

- **JSONL (Interchange)**: Used for data import/export and manual auditing.
- **DuckDB (Operational)**: Used for the live TUI and reporting.

Even in DuckDB, the system maintains an **event-sourced** approach where possible, appending observations rather than overwriting historical state. This ensures:
- Full audit trail preserved
- Resilience to partial failures
- High-performance analytical queries (DuckDB excels at processing large event tables)

### Observations

Balebos distinguishes between assets (inventory) and observations (checks, probes).

- Inventory represents what exists
- Observations represent what was seen at a given time

Examples:
- domain → inventory
- subdomain → inventory (discovered via probes)
- http_check → observation (HTTP status, response time, network errors)

This separation ensures that the system can track both what should exist (the inventory)
and actual observations over time, providing a foundation for detecting drift and anomalies.

### Network Error Capture

HTTP checks capture detailed network error information:

- **DNS Errors** - dns_not_found, dns_timeout
- **TLS Errors** - certificate_error, tls_handshake_failed
- **Connection Errors** - connection_refused, connection_timeout
- **Protocol Errors** - http_error, invalid_response

Each error includes detailed error messages for debugging and reporting.

### Invariants

Balebos enforces simple operational invariants:

- every asset should be observed periodically
- unknown sources are suspicious by default
- low-frequency observations reduce confidence
- absence of data is treated as a signal

Violations of these invariants are reported as findings.

### Findings

Findings are the primary mechanism for tracking operational issues in Balebos. Unlike ephemeral issues that were previously printed and discarded, findings are:

- **Persisted** - Stored as append-only records in the inventory
- **Queryable** - Can be listed, filtered, and analyzed over time
- **Auditable** - Complete lifecycle history (created, acknowledged, resolved)
- **Deduplicated** - Same check + target combination won't create duplicates
- **Auto-resolving** - Close automatically when conditions are fixed

#### Finding Lifecycle

```
Created → Open → Acknowledged → Resolved
   ↓
Auto-resolved (when condition fixed)
```

1. **Created** - A check detects an issue and creates a finding
2. **Open** - Default state, requires attention
3. **Acknowledged** - Seen but not yet fixed (suppresses alerts)
4. **Resolved** - Fixed either manually or automatically

#### Severity Levels

- **Critical** - Immediate action required (expired certs, outages)
- **High** - Address soon (certs expiring, stale production assets)
- **Medium** - Worth investigating (low confidence, unknown sources)
- **Low** - Minor issues (missing checks)
- **Info** - Informational observations

#### Categories

Findings are categorized to support different operational workflows:
- **Security** - Certificate issues, TLS problems
- **Compliance** - Policy violations
- **Financial** - Cost-related concerns
- **Lifecycle** - Stale assets, deprecation
- **Operational** - HTTP errors, DNS issues
- **Governance** - Ownership, tagging

### Anomaly Detection

Rather than just collecting data, Balebos actively analyzes it for anomalies:
- Items near expiration or not seen recently
- Items with low confidence (few observations)
- Items from unknown or untrusted sources
- Items with suspicious patterns
- Unexpected changes in the inventory

Detected anomalies are persisted as findings with appropriate severity and category.

### Synthesis for Decision Making

Balebos compresses information into actionable insights:
- The `report` command synthesizes data from multiple sources
- Changes are highlighted for attention
- Issues are categorized and prioritized
- A clear "needs attention" flag indicates action requirements

### Tool Orchestration & Concurrency

Balebos acts as a high-performance orchestrator for specialized OSINT tools. To maximize efficiency, it uses a **streaming batch architecture**:

1.  **Bulk Input**: Instead of calling tools per-domain, Balebos launches a single persistent process for the entire target list.
2.  **Standard Streams**: Data is fed via `stdin` and results are captured from `stdout` in real-time.
3.  **Threaded Processing**: A dedicated background thread manages the input stream, while the main thread parses and saves incoming JSON objects line-by-line.
4.  **Native Speed**: This approach allows tools like `dnsx` and `httpx` to use their internal Go-based concurrency (routines) at full capacity, processing thousands of assets in seconds.

## Implementation Details

### Storage Format

Data is stored as newline-delimited JSON (JSONL) with support for multiple record types:

```json
{"item_type":"domain","id":"uuid","name":"example.com","discovered_at":"timestamp"}
{"item_type":"subdomain","id":"sub:api.example.com","name":"api.example.com","source":"subfinder"}
{"item_type":"dns_record","target":"acme-corp.com","a":["203.0.113.10"],"aaaa":["2001:db8::1"],"cname":[]}
{"item_type":"http_check","target":"acme-corp.com","status":200,"final_url":"https://www.acme-corp.com/","redirect_chain":[{"url":"https://acme-corp.com","status":301,"redirect_to":"https://www.acme-corp.com/"}],"tech":["Nginx","HTTP/3"],"webserver":"nginx","title":"Acme Corp"}
```

**Record Types:**
- `domain` - Root domains
- `subdomain` - Discovered subdomains
- `http_check` - HTTP probe results and network observations (including tech stack, server fingerprints, and redirect chains via `httpx`)

This format offers:
- Human-readability for debugging
- Easy processing with standard tools (grep, jq, etc.)
- Schema flexibility for extension
- Simple append operations
- Support for heterogeneous data (multiple record types in single file)

### Error Handling

Balebos uses Rust's `Result` type extensively to handle errors gracefully:
- I/O errors are propagated to the caller
- Parse errors for individual lines don't crash the program
- Invalid data is flagged rather than rejected
- Each component returns meaningful errors

### External Tools Integration

Rather than reinventing the wheel, Balebos integrates with proven tools:
- subfinder for subdomain enumeration
- HTTP client for web service status checks
- (future) other specialized tools for specific tasks

Integration is done via subprocess execution and output parsing for external tools, and via direct API calls for built-in tooling, following Unix philosophy.

### Exit Semantics

Balebos uses exit codes to signal system state:

- 0 → all systems normal
- 1 → general error
- 2 → invalid argument
- 3 → storage error
- 20 → critical issues detected
- 21 → warning issues detected
- 30 → HTTP error
- 31 → HTTP client error (4xx)
- 32 → HTTP server error (5xx)
- 40 → probe error
- 41 → external tool error

This allows integration with automation and alerting systems for specific response types.

## Findings System

### Motivation

The original `check` command printed issues to stdout and exited. This had several limitations:
- No historical record of what was detected
- No way to track if issues were getting better or worse
- No acknowledgment or resolution workflow
- Report module had to parse strings to count issues

The findings system replaces ephemeral issues with persisted records.

### Implementation

**Finding Structure:**
```rust
pub struct Finding {
    pub id: String,                    // UUID
    pub severity: FindingSeverity,     // Info, Low, Medium, High, Critical
    pub category: FindingCategory,     // Security, Compliance, etc.
    pub check_name: String,            // Which check produced this
    pub target: String,                // Affected asset
    pub message: String,               // Human-readable description
    pub created_at: DateTime<Utc>,
}
```

**Lifecycle Events:**
```rust
pub struct FindingEvent {
    pub finding_id: String,
    pub event_type: FindingEventType,  // Acknowledged, Resolved
    pub created_at: DateTime<Utc>,
}
```

### Auto-Resolution

Findings automatically resolve when the underlying condition is fixed:
- Staleness finding → Resolved when asset seen recently
- HTTP error finding → Resolved when HTTP returns success
- Low confidence finding → Resolved when multiple observations exist

This ensures the findings list reflects current reality without manual cleanup.

## v0.2.0 Features

### Domain Hierarchy Visualization

The `tree` command provides a hierarchical view of your infrastructure:
- Displays root domains and discovered subdomains in a tree structure
- Shows multi-level nesting relationships
- Helps identify complex subdomain organizations
- Useful for understanding infrastructure topology

### Domain Status Reporting

The `status` command analyzes HTTP probe results across all assets:
- Categorizes domains by health status (alive, client errors, server errors, network errors)
- Shows statistics (count and percentage) for each category
- Displays response times and error details
- Identifies portfolio health at a glance

**Export Capabilities:**
- CSV export for spreadsheet analysis
- Markdown export for documentation and version control
- HTML export with styled formatting for web viewing
- All formats include comprehensive data and timestamps

### Batch Processing

The `batch` command combines multiple operations for efficiency:
- Scans domain and adds to inventory
- Discovers subdomains using subfinder
- Performs HTTP checks on all discovered domains
- Single command for complete reconnaissance

## Extension Points

Balebos can be extended in several ways:

1. **New Asset Types**
   - Add new item_type values
   - Create appropriate storage and query logic
   - Update reporting to include new types

2. **New Data Sources**
   - Create new probe commands
   - Integrate with external tools
   - Record source in inventory items

3. **New Analysis Rules**
   - Add checks to the check_inventory function
   - Define new issue types and severity levels
   - Update reporting to highlight new issues

4. **Custom Filtering**
   - Define patterns in balebos.ignore
   - Create specific ignore rules for your environment
   - Filter noisy or irrelevant assets without modifying storage

## Design Decisions and Trade-offs

### Why DuckDB and JSONL?

The project evolved from a pure JSONL storage to a DuckDB-backed system:

- **JSONL** remains for its simplicity, human-readability, and ease of backup. It is the perfect format for piping data between CLI tools.
- **DuckDB** was introduced to solve performance bottlenecks in the TUI and reporting. It provides sub-second query times on large datasets without the overhead of a full-blown database server (PostgreSQL/MySQL).
- The **Hexagonal Architecture** allows users to choose the backend that fits their needs (e.g., pure JSONL for tiny embedded systems, DuckDB for large-scale recon).

### Why append-only?

- Simpler logic (no need to handle updates)
- Complete historical record
- Better operational characteristics
- More resilient to failures

### Why not normalize everything aggressively?

- Preserving original data maintains flexibility
- Different queries may need different normalizations
- Interpretation should happen at query time
- Source data should remain as close to original as possible

### Why command-line over web interface?

- Follows Unix philosophy of tools that do one thing well
- Easily automated and integrated with other tools
- Lower overhead and dependencies
- Natural fit for infrastructure pipelines