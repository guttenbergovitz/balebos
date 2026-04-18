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
   - **Adapter (JSONL)**: Interchange and audit trail backend using append-only text files
   - **Adapter (Parquet)**: Primary storage using Polars-based columnar format with in-memory buffering
   - Per-record-type Parquet files with automatic schema inference
   - In-memory buffer with configurable batch flushing (default: 100 records)
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
   - **`app/`** — Modular state management with separate modules for each view (inventory, findings, organisation, scanning)
   - **`state/mod.rs`** — Core application state machine managing navigation, input modes (Normal, Editing, Filtering), and background orchestration
   - **`state/findings.rs`** — Findings view with lifecycle management, filtering, and severity prioritisation
   - **`model.rs`** — Loads and aggregates inventory data into `AssetState` groups (root domains with subdomains), computes global statistics, and detects external redirects
   - **`ui.rs`** — Rendering layer: header with live stats, multi-tab navigation (Inventory, Organisation, Projects, Findings), detail inspector, filter bar, and keyboard help
   - Background scanning runs in a separate thread with progress updates piped back via shared `scan_status` — no stdout pollution in alternate screen mode

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

Balebos combines the simplicity of append-only text logs with the performance of columnar storage:

- **JSONL (Interchange)**: Used for data import/export, manual auditing, and pipeline integration with external tools.
- **Parquet (Operational)**: Polars-based columnar storage used for the live TUI, reporting, and analytical queries.

The system maintains an **event-sourced** approach throughout, appending observations rather than overwriting state:
- Full audit trail preserved in Parquet format
- Resilience to partial failures and interruptions
- High-performance queries on large event tables via Polars columnar processing
- In-memory buffering with automatic batch flushing ensures fast writes without overwhelming I/O

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

Data is stored in Parquet format with per-record-type files. The system maintains a unified Utf8 schema across all fields for flexibility and extensibility.

**File Structure:**
```
data/
├── domain.parquet          # Root domains
├── subdomain.parquet       # Discovered subdomains
├── dns_record.parquet      # DNS resolution observations
├── http_check.parquet      # HTTP probe results
├── finding.parquet         # Persisted issues
├── finding_event.parquet   # Finding lifecycle events
├── org_brand.parquet       # Organisation brands
├── org_company.parquet     # Companies and contractors
├── org_project.parquet     # Projects and cost centres
└── ... (additional record types as needed)
```

**Data Flow:**
1. Records arrive as JSON objects (e.g., from CLI, external tools, API)
2. In-memory buffer accumulates records (default: 100 record threshold)
3. Buffer automatically flushes to Parquet when threshold reached
4. Polars schema is inferred from the first JSON object of each type
5. Fields are stored as UTF-8 text for schema flexibility

**Benefits of This Approach:**
- **Human-readable interchange** - JSONL format used for imports/exports and CLI integration
- **High-performance queries** - Parquet columnar format enables fast analytical queries via Polars
- **Schema flexibility** - Unified Utf8 schema allows new fields without migration
- **Batch efficiency** - In-memory buffering reduces I/O overhead
- **Audit trail** - Append-only records preserve complete history

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

### Why Parquet and JSONL?

The project evolved from pure JSONL storage to a Parquet-backed system with Polars:

- **JSONL** remains for its simplicity, human-readability, and ease of backup. It's the perfect format for piping data between CLI tools, importing/exporting, and cross-system integration.
- **Parquet** was introduced to solve performance bottlenecks in the TUI and reporting. Combined with Polars, it provides:
  - Sub-second query times on large datasets
  - Columnar compression for efficient storage
  - No server dependency (unlike PostgreSQL/MySQL)
  - In-memory processing for local analysis
  - Schema inference for field flexibility
- The **Hexagonal Architecture** with the `StorageEngine` trait allows flexibility: code is decoupled from storage choice, making it straightforward to add new backends if needed.

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

### Why command-line first?

- Follows Unix philosophy of tools that do one thing well
- Easily automated and integrated with other tools
- Lower overhead and dependencies
- Natural fit for infrastructure pipelines

That said, Balebos does provide a **static web dashboard** for stakeholders who need a browser-based view. It is generated from the same Parquet data, requires no server-side code, and can be deployed to any static hosting. The CLI remains the primary interface; the web dashboard is a read-only export for sharing.