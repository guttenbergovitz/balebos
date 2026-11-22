# 0003 — Single-crate structure

**Status:** Accepted

## Context

The codebase is ~20.6k lines with one binary surface (CLI + TUI) and one
persistence backend. A workspace split was considered to enforce layering, but
the initial audit found no concrete need for multiple crates: no independently
versioned libraries, no divergent release cycles, no shared components consumed
elsewhere.

## Decision

Balebos remains a single Cargo package: a library (`src/lib.rs`) with a thin
executable (`src/main.rs`). Internal module boundaries
(`domain`, `inventory`, `storage`, `app`, `cli`, `tui`, and later `discovery`,
`inspection`, `drift`, `assessment`, `export`) express the architecture.
Dependency direction is enforced by convention and review, not by crate
boundaries. This decision is revisited only if a concrete need emerges (an
independently consumable library, a plugin ABI, or build-time isolation with
measured benefit).

## Consequences

- No workspace ceremony, no artificial `*-core` crates, faster builds.
- Layer violations remain possible in principle; the `balebos-architecture-review`
  skill and CI-level review discipline are the enforcement mechanism.
- Module renaming during migration is cheap because there is no public crate API
  to break until 2.0 stabilises.
