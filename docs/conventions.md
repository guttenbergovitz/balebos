# Engineering conventions

British English everywhere. Precise, plain language; explain purpose, constraints,
invariants and trade-offs — never narrate obvious code.

## Rust standards

### Error handling

- `Result<T, BalebosError>` for fallible operations; propagate with `?`.
- No arbitrary `.unwrap()`/`.expect()` in production code. Reserve `.expect()` for
  provable invariants with a brief explanation of why they cannot fail. Tests may
  unwrap freely (`clippy.toml` allows it).
- No silent fallbacks on recoverable errors; no turning parse failures into valid
  default domain objects. Corrupt or unknown input is surfaced, not skipped quietly.
- Errors must be typed where callers make structured decisions; avoid generic
  string errors at subsystem boundaries.

### Types and invariants

- Validated newtypes at subsystem boundaries (IDs, names, states) where primitives
  would permit invalid values.
- Enums over loosely related booleans; exhaustive matches; never map unknown
  variants onto unrelated valid values.
- Distinguish parse failure from a valid observation containing an error.
- Provenance is mandatory for observations: what was inspected, which inspection,
  when, which source produced it, what result, whether it is complete and trusted.

### Ownership and borrowing

- Explicit ownership; prefer borrowing; clone and convert intentionally, not to
  silence the borrow checker. No `Arc<Mutex<_>>` as a substitute for ownership design.

### Traits and abstractions

- Concrete types until multiple implementations or an explicit testing boundary
  justifies abstraction. No speculative traits, generic frameworks or premature
  parameterisation.

### Performance

- Measure before optimising. Consider allocations, I/O, batch size, process
  spawning, lock contention and streaming behaviour for hot paths. No fashionable
  caching or concurrency without a measured reason.

### Unsafe

- Safe Rust by default. `unsafe` requires justification, a documented safety
  invariant, focused tests, and an explanation of why safe alternatives fail.

### Dependencies

- Every crate needs a reason: maintenance status, licence, security exposure,
  compatibility and compile cost. Prefer the standard library for clear solutions;
  do not reimplement mature ecosystem functionality to shave dependencies.
- Major dependency or architectural commitments are recorded as ADRs.

## Testing

- TDD where practical; tests target guarantees, not implementation trivia.
- Deterministic and offline: fixtures and controlled local processes. No
  network-dependent default tests; no sleeps to mask races. Live-network
  compatibility tests live in a separate, explicitly invoked suite.
- Required categories: unit (validation, normalisation, identity, state
  transitions, rules, drift classification), integration (CLI, storage,
  import/export round trips, adapters, TUI state), failure (corrupt input,
  interrupted writes, timeouts, partial scans, missing tools, cancellation),
  property-based where invariants benefit, benchmarks for storage and comparison
  hot paths.
- Coverage is judged by which violated invariants would cause wrong operator
  decisions or lost evidence — not by percentage.

## Git

- Git Flow: `main` (release-ready), `develop` (integration), `feature/*`,
  `release/*`, `hotfix/*`.
- Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`, `test:`,
  `build:`), SemVer, atomic single-concern commits, British English messages.
- Never commit secrets, build output, or `.specs/` content. No force-pushing, no
  rewriting divergent branches, no unverified work committed as complete.
