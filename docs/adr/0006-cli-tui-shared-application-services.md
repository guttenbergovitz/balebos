# 0006 — CLI/TUI shared application services

**Status:** Accepted

## Context

In 1.x, the TUI (`src/tui/`) reimplements business logic in its state modules
(findings lifecycle, inventory aggregation, organisation workflows) in parallel
with the CLI handlers (`src/cli/handlers/`). The two can diverge silently; the
audit lists this as a top architectural risk. Meanwhile the TUI is a primary
operator interface and must not regress.

## Decision

CLI and TUI are two front ends over shared application services (`app` layer).
Domain and assessment logic lives once, in library modules; front ends format and
interact. The TUI contains rendering and interaction state machines only — no
independent business rules. Long-running work (scans, storage) runs off the
terminal event loop with explicit progress channels, and terminal restoration is
guaranteed on error paths where feasible.

Existing TUI screens are kept until improved replacements are implemented and
verified (no broad UI rewrite); org screens retire with ADR 0008.

## Consequences

- Phase 5 refactors handlers behind application services with regression tests
  asserting CLI/TUI agreement on the same operations (e.g. drift comparison).
- TUI state machines (navigation, filtering, workflow transitions) are unit
  tested terminal-independently, as they already are in 1.x.
- New features are implemented service-first; front ends consume.
