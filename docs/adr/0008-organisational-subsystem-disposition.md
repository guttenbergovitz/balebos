# 0008 — Organisational subsystem disposition

**Status:** Accepted

## Context

1.x contains a full governance subsystem (`src/org/`): Brand → Company →
Project → Asset hierarchy, Person records, role assignments (Owner/Manager/
Technician), contractor assignments, asset importance, domain registrations —
8 of 18 storage record types exist solely for it (audit §8). The product
boundary (ADR 0002) assigns organisational hierarchy, people/role administration
and approval workflows to higher-level systems; retaining the subsystem in
Balebos duplicates enterprise data without standalone value and drags the TUI
(org/projects tabs) and CLI (`org *` commands) along with it.

## Decision

Retire the governance subsystem; retain lightweight local metadata.

- **Removed**: Brand/Company/Project/Person entities, role and contractor
  assignments, the `org *` CLI family, org/projects TUI tabs and workflows.
- **Retained (local, lightweight)**: asset tags (existing), asset lifecycle
  events (existing), plus notes, an asset-importance flag and a plain owner
  reference (string) for standalone work.
- **Data**: before removal, all `org_*` records are exported to JSONL; a
  one-time import maps asset → project/owner names into the retained plain
  fields. Nothing is silently discarded (see `docs/migration-v2.md`).

## Consequences

- `isocountry` and 8 `ItemType` variants become removable; TUI loses two tabs
  and two workflow state machines; CLI loses a command family. ~3–4k LOC
  estimated.
- Phase 5 executes the removal with the migration/export tooling above; until
  then the subsystem stays functional but frozen (no new features).
- Owner/contractor visibility survives in the retained fields, so standalone
  operators keep the one thing that matters locally: "who do I talk about this
  asset".
