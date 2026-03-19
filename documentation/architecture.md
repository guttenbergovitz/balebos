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
   - Handles persistence in append-only JSONL
   - Provides read and write operations
   - Implements resilient error handling

3. **Application Layer**
   - Contains business logic and use cases
   - Orchestrates domain and storage operations
   - Implements interpretation and analysis

4. **CLI Layer**
   - Handles command-line interface
   - Parses arguments and options
   - Presents results to the user

### Data Flow

```
CLI → Application → Domain → Storage
              ↑         ↑
              └────Storage
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

### Normalization

Balebos stores both raw and normalized data:
- Domain names are converted to lowercase
- Protocol prefixes (http://, https://) are removed
- www prefixes are removed
- Trailing slashes are removed

Normalization is stored alongside raw input, not instead of it.
Interpretation still happens at query time, using the normalized form while
preserving the original data for reference and auditing.

### Append-Only Storage

All data in Balebos is stored append-only, meaning:
- New records are added, never updated or deleted
- Historical data is preserved indefinitely
- Each record has its own timestamp
- Duplicates are allowed and handled at query time

This approach offers several benefits:
- Simpler write operations (no conflicts)
- Complete audit trail
- Resilience to partial failures
- Better concurrency characteristics

### Observations

Balebos distinguishes between assets (inventory) and observations (checks, probes).

- Inventory represents what exists
- Observations represent what was seen at a given time

Examples:
- domain → inventory
- http_check → observation (HTTP status, response time)
- subdomain_scan → observation

This separation ensures that the system can track both what should exist (the inventory)
and actual observations over time, providing a foundation for detecting drift and anomalies.

### Invariants

Balebos enforces simple operational invariants:

- every asset should be observed periodically
- unknown sources are suspicious by default
- low-frequency observations reduce confidence
- absence of data is treated as a signal

Violations of these invariants are reported as issues.

### Anomaly Detection

Rather than just collecting data, Balebos actively analyzes it for anomalies:
- Items near expiration or not seen recently
- Items with low confidence (few observations)
- Items from unknown or untrusted sources
- Items with suspicious patterns
- Unexpected changes in the inventory

### Synthesis for Decision Making

Balebos compresses information into actionable insights:
- The `report` command synthesizes data from multiple sources
- Changes are highlighted for attention
- Issues are categorized and prioritized
- A clear "needs attention" flag indicates action requirements

## Implementation Details

### Storage Format

Data is stored as newline-delimited JSON (JSONL):
```
{"item_type":"domain","id":"uuid","name":"example.com","discovered_at":"timestamp"}
{"item_type":"subdomain","id":"sub:api.example.com","name":"api.example.com","source":"subfinder"}
```

This format offers:
- Human-readability for debugging
- Easy processing with standard tools (grep, jq, etc.)
- Schema flexibility for extension
- Simple append operations

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

### Why JSONL instead of a database?

- Simplicity of implementation and operations
- No database dependencies to manage
- Human-readable format for debugging
- Easy backup and version control
- Sufficient performance for expected data volumes

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