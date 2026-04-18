# Balebos Documentation

Welcome to the Balebos documentation. This is your central index for understanding the system, using it effectively, and contributing to its development.

## Getting Started

New to Balebos? Start here:

1. **[Getting Started Guide](guides/getting-started.md)** - Installation, quick start, and basic workflows
2. **[Usage Reference](guides/usage.md)** - Detailed command documentation

## User Guides

Comprehensive guides for specific features and workflows:

### Core Features
- **[Findings Management](guides/findings.md)** - Tracking and managing operational issues
- **[Asset Lifecycle](guides/asset-lifecycle.md)** - Formal states for assets (Active, Deprecated, etc.)
- **[Risk Scoring](guides/risk-scoring.md)** - Understanding risk assessment and prioritisation
- **[Organisation Model](guides/organization.md)** - Tracking ownership, teams, and accountability

### Specialised Topics
- **[Reconnaissance Pipeline](guides/recon-pipeline.md)** - How assets are discovered and probed
- **[Tag Management](guides/tag-management.md)** - Organising and filtering assets with tags
- **[Web Dashboard](guides/web-dashboard.md)** - Exporting static site for stakeholders

## Technical Documentation

For developers and those curious about the internals:

### Architecture & Design
- **[Architecture Overview](architecture.md)** - System design, components, and data flow
- **[Design Decisions](architecture.md#design-decisions-and-trade-offs)** - Why we made certain choices (Parquet, JSONL, append-only storage, etc.)

### Data & Storage
- **[Data Model Specification](specs/data-model.md)** - Complete schema for all record types
  - Domains, subdomains, and observations
  - Findings and lifecycle events
  - Organisation (brands, companies, projects, people)
  - DNS, HTTP, TLS observations
  - Domain registration data

### Decision Records
- **[ADR-001: Persisted Findings](adr/001-persisted-findings.md)** - Why findings are stored as append-only records

## Quick Reference

### Common Tasks

**Set up infrastructure monitoring:**
```bash
balebos scan example.com
balebos probe subdomains example.com
balebos probe http example.com
balebos ui
```

**Check for issues:**
```bash
balebos finding run          # Detect issues
balebos finding list         # List findings
balebos risk report          # Prioritise by risk
```

**Export web dashboard:**
```bash
./build-web.sh               # Export data and build static site
# Output: web/public/
```

**Manage organisational context:**
```bash
balebos org brand add "My Company"
balebos org company add "Platform Team" DEU --brand <id>
balebos org project add "Website" --company <id>
balebos org assign asset api.example.com --project <id>
```

**Track lifecycle:**
```bash
balebos asset lifecycle set api.example.com active
balebos asset lifecycle list
```

### Command Index

- **inventory**: `balebos scan`, `balebos list`, `balebos tree`
- **probing**: `balebos probe subdomains`, `balebos probe dns`, `balebos probe http`, `balebos batch`
- **analysis**: `balebos check`, `balebos status`, `balebos diff`, `balebos report`
- **findings**: `balebos finding run`, `balebos finding list`, `balebos finding acknowledge/resolve`
- **risk**: `balebos risk report`, `balebos risk top`, `balebos risk show`
- **asset management**: `balebos asset lifecycle set/show/history/list`
- **organisation**: `balebos org brand/company/person/project add/list`
- **tags**: `balebos tag add/list/remove`
- **web**: `balebos web export`, `./build-web.sh`
- **TUI**: `balebos ui`

See [Usage Reference](guides/usage.md) for complete documentation of all commands.

## Architecture at a Glance

Balebos is built in layers:

```
CLI / TUI
    ↓
Application Layer (use cases, orchestration)
    ↓
Domain Layer (validation, business logic)
    ↓
Storage Layer (Parquet + JSONL)
```

**Storage Strategy:**
- **Parquet** - High-performance columnar storage for TUI and reporting
- **JSONL** - Human-readable interchange format for imports/exports
- **Polars** - Fast in-memory processing
- **Append-only** - Full audit trail, resilient to failures

Read [Architecture Overview](architecture.md) for deep dive.

## Key Concepts

### Inventory
Your centralised registry of all infrastructure assets. Preserves historical data, tolerates noise, separates raw data from interpretation.

### Findings
Persisted issues with formal lifecycle: Created → Open → Acknowledged → Resolved. Auto-resolves when underlying conditions improve. Never lost, fully auditable.

### Observations
Data collected at specific moments (DNS checks, HTTP responses, TLS certs). Separate from inventory (what exists) to enable trend analysis.

### Findings × Risk
Risk scores combine findings, observation age, and lifecycle state to help prioritise which assets need attention.

### Organisation
Brands → Companies → Projects → Assets. Maps your structure to infrastructure, enabling accountability and cost tracking.

## Project Structure

```
documentation/
├── README.md                 ← You are here
├── architecture.md           # System design and philosophy
├── guides/
│   ├── getting-started.md   # Installation and quick start
│   ├── usage.md             # Command reference
│   ├── findings.md          # Issue tracking
│   ├── asset-lifecycle.md   # Asset states
│   ├── risk-scoring.md      # Risk assessment
│   ├── organization.md      # Ownership and teams
│   ├── tag-management.md    # Tagging and filtering
│   └── recon-pipeline.md    # Discovery and probing
├── specs/
│   └── data-model.md        # Storage schema
├── adr/
│   └── 001-persisted-findings.md
└── future/                  # Planning documents
    ├── big-picture.md
    ├── evolution-roadmap.md
    └── ...
```

## Contributing to Documentation

Documentation is prose, not code comments. When you:
- Add a new feature, document it in the appropriate guide
- Change behaviour, update architecture.md
- Add a record type, update specs/data-model.md
- Make architectural decisions, consider creating an ADR

Keep documentation:
- **Accurate** - Reflects current code behaviour
- **Practical** - Examples that actually work
- **Scannable** - Headings, lists, code blocks
- **British English** - Casual, smart, no-nonsense tone
- **Timeless** - Avoid "recent" or "upcoming", be specific

## Troubleshooting & FAQ

Common issues? Check the relevant guide:
- **Installation problems** → [Getting Started](guides/getting-started.md#troubleshooting)
- **Command usage** → [Usage Reference](guides/usage.md)
- **Findings not working** → [Findings Guide](guides/findings.md#troubleshooting)
- **Risk scores seem wrong** → [Risk Scoring Guide](guides/risk-scoring.md)

## Further Reading

- [README.md](../README.md) - Project overview and quick start
- [CHANGELOG.md](../CHANGELOG.md) - Version history and changes
- [AGENTS.md](../AGENTS.md) - AI assistant guidance

## Support

- Check command help: `balebos --help` or `balebos <command> --help`
- Review [Getting Started](guides/getting-started.md#troubleshooting)
- Read relevant guide for your feature
- Check [CHANGELOG](../CHANGELOG.md) to see what's new

---

**Last updated:** 2026-04-18
**Status:** Actively maintained
