# Agent Guidelines for Balebos

Instructions for AI coding agents working in the Balebos repository. This file is
the authoritative short guide; durable detail lives under `docs/`.

## Language

Use **British English (en-GB)** consistently across documentation, comments,
Rustdoc, user-facing and error messages, ADRs, and commit messages
(*normalise*, *behaviour*, *artefact*, *initialise*). Write plain, direct prose
that explains purpose and invariants, not narrated syntax.

## What Balebos is

A **local-first infrastructure discovery, inspection and drift-detection tool**.
It answers: What exists? What is running? What has changed? What needs attention?
What evidence supports these conclusions?

## What Balebos is not

- Not a SIEM, not a monitoring platform, not an enterprise CMDB.
- Not a security scanner: no SCA, CVE/CVSS/KEV/EPSS enrichment, secret scanning,
  SBOM or licence compliance — that is Angry Turnip's boundary. Do not reimplement it.
- Not a governance system: no Brand → Company → Project → Person hierarchies,
  approval workflows or business-risk acceptance. Lightweight local grouping,
  tags, notes, importance and owner references are sufficient.
- Not a distributed platform: no server, broker or central database. It must run
  standalone.

## Product invariants (non-negotiable)

1. **Evidence before interpretation.** Observations are facts about an inspection
   attempt; findings are derived assessments. Interpretation never overwrites an
   observation.
2. **Unknown is not healthy.** Distinguish success, failure, skipped, unsupported,
   incomplete and unverified. A failed scan never implies assets disappeared; a
   partial scan is never presented as complete.
3. **Local-first.** Core operations work without hosted services beyond the
   inspected targets and explicitly configured external scanners.
4. **Fails visibly.** Silent data loss, silent record skipping and fabricated
   defaults are blocking defects, not warnings.

## Architecture

Single Cargo package: library + thin binary. Dependency direction:

```
cli, tui  →  app (workflows)  →  domain, drift, assessment  →  storage
                     ↑
        discovery/inspection adapters (subprocess tools)
```

- `domain` never depends on CLI, TUI, storage implementations or scanner processes.
- TUI and CLI share application services; the TUI contains no independent business rules.
- Prefer concrete types. Introduce traits only at genuine variation or testing
  boundaries; no speculative abstraction frameworks.

## Rust standards

Follow `docs/conventions.md`. In particular:

- No arbitrary `.unwrap()`/`.expect()` in production code; typed errors with `?`.
- No gratuitous `.clone()` to silence the borrow checker; design ownership properly.
- Validated newtypes at subsystem boundaries; enums over loosely related booleans;
  never map unknown enum variants onto unrelated valid values.
- No `unsafe` without justification, documented invariant and focused tests.
- Every dependency needs a reason; prefer the standard library for clear solutions.

## Workflow

- Canonical skills live under `.agents/skills/` (`balebos-feature-slice`,
  `balebos-architecture-review`, `balebos-security-audit`, `balebos-storage-review`,
  `balebos-release-check`). Use them.
- Temporary plans, scratchpads and working specifications live **only** under
  `.specs/` (gitignored). Never scatter temporary documents across the repository.
- Durable decisions belong in `docs/adr/NNNN-short-title.md` (Status / Context /
  Decision / Consequences). Keep accepted ADRs immutable except supersession.

## Verification

- Quality gate: `task check` (fmt, clippy `-D warnings`, rustdoc `-D warnings`,
  tests). Everything must pass before committing.
- Tests must be deterministic and offline. Use fixtures and controlled local
  processes; no sleeps to mask races; no network-dependent default tests.
- Pre-existing failures are reported as pre-existing, never hidden.

## Git discipline

- Git Flow: `main` (release-ready), `develop` (integration), `feature/*`,
  `release/*`, `hotfix/*`. Conventional Commits, SemVer, atomic commits,
  British English messages.
- Never commit secrets, build output or `.specs/` content. Never force-push or
  rewrite divergent branches. Do not commit unverified work as complete.
