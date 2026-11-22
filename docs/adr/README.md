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
| [0007](0007-local-persistence.md) | Local persistence and migration | Superseded by 0012 + 0013 |
| [0008](0008-organisational-subsystem-disposition.md) | Organisational subsystem disposition | Accepted |
| [0009](0009-drift-detection-and-snapshot-semantics.md) | Drift detection and snapshot semantics | Proposed |
| [0010](0010-versioned-integration-contracts.md) | Versioned machine-readable integration contracts | Proposed |
| [0011](0011-infrastructure-relationship-graph.md) | Infrastructure relationship graph (future phase) | Proposed |
| [0012](0012-sqlite-operational-storage.md) | SQLite as authoritative operational storage | Accepted |
| [0013](0013-parquet-evidence-archive.md) | Parquet as immutable evidence archive (optional) | Accepted |
| [0014](0014-evidence-segment-sealing.md) | Evidence segment sealing and verification | Accepted |
| [0015](0015-hash-chain-and-anchoring.md) | Hash chain and external anchoring (future) | Proposed |
| [0016](0016-remote-worm-archival.md) | Optional remote WORM archival (contract only) | Proposed |
| [0017](0017-dependency-and-build-time-policy.md) | Dependency and build-time policy | Accepted |
| [0018](0018-legacy-parquet-migration.md) | Legacy Parquet migration | Accepted |

ADR 0007 (local persistence) was superseded by 0012 and 0013 and is
retained as historical record of the interim engine's verified defects.

The legacy ADR `documentation/adr/001-persisted-findings.md` is retained as
historical; it remains valid background for the findings subsystem.
