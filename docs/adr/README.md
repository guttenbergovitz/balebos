# Architecture Decision Records

Short factual records of significant architectural decisions. Format: **Status**,
**Context**, **Decision**, **Consequences**. Nothing else — no invented
stakeholders, approval boards or rollout theatre.

- Statuses: `Proposed`, `Accepted`, `Superseded by NNNN`.
- Accepted ADRs are immutable apart from supersession metadata.
- Files: `NNNN-short-descriptive-title.md`, numbered sequentially.
- Unresolved decisions stay `Proposed` and list the evidence required to decide.

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-product-scope-and-non-goals.md) | Product scope and explicit non-goals | Accepted |
| [0002](0002-separation-from-angry-turnip-and-higher-level-systems.md) | Separation from Angry Turnip and higher-level systems | Accepted |
| [0003](0003-single-crate-structure.md) | Single-crate structure | Accepted |
| [0004](0004-asset-observation-assessment-separation.md) | Asset, observation and assessment separation | Accepted |
| [0005](0005-scan-completeness-and-uncertainty-semantics.md) | Scan completeness and uncertainty semantics | Accepted |
| [0006](0006-cli-tui-shared-application-services.md) | CLI/TUI shared application services | Accepted |
| [0007](0007-local-persistence.md) | Local persistence and migration | Proposed |
| [0008](0008-organisational-subsystem-disposition.md) | Organisational subsystem disposition | Accepted |
| [0009](0009-drift-detection-and-snapshot-semantics.md) | Drift detection and snapshot semantics | Proposed |
| [0010](0010-versioned-integration-contracts.md) | Versioned machine-readable integration contracts | Proposed |

The legacy ADR `documentation/adr/001-persisted-findings.md` is retained as
historical; it remains valid background for the findings subsystem.
