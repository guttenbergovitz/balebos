---
name: balebos-feature-slice
description: >-
  Spec-driven TDD workflow for implementing a small, verifiable feature slice in
  Balebos. Use when adding or modifying functionality across domain, discovery,
  inspection, storage, assessment, CLI or TUI modules.
---

# Balebos Feature Slice Workflow

Follow this sequence for every non-trivial code change so work stays small,
verifiable and inside the product boundary.

## 1. Inspect before coding

- Read the affected modules and the relevant ADRs in `docs/adr/`.
- If the change introduces new behaviour or non-obvious invariants, write a
  concise working specification in `.specs/` first. Never scatter temporary
  plans outside `.specs/`.
- Confirm the change respects boundaries: `domain` knows nothing of CLI, TUI,
  storage engines or scanner processes; CLI and TUI share application services.

## 2. Define expected behaviour and invariants

- State what is observed (evidence) and what is concluded (assessment) —
  they are different concepts (ADR 0004).
- Decide explicitly how incomplete, failed, skipped or unsupported inspections
  are represented (ADR 0005). "Not observed" must never become "disappeared".

## 3. Write focused tests first

- Unit or integration tests defining expected behaviour, boundaries and failure
  modes before or alongside implementation.
- Cover: invalid input, partial scans, missing external tools, corrupt data,
  cancellation. Tests may `.unwrap()`; production code must not.

## 4. Implement the smallest coherent change

- Model invalid states out of existence with domain types and enums; never map
  unknown variants onto unrelated valid values.
- Propagate errors with `Result` and `?`; no silent fallbacks, no fabricated
  defaults for unknown/corrupt records.
- Prefer borrowing over gratuitous `.clone()`; no `Arc<Mutex<_>>` as an
  ownership shortcut.
- Plain, direct en-GB docstrings explaining purpose and invariants.
- New dependencies need justification (maintenance, licence, compile cost).

## 5. Run the local quality gate

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Zero warnings, zero failures. Pre-existing failures are reported as such, never
hidden.

## 6. Review architectural boundaries

- Does anything in `domain` import CLI/TUI/storage-implementation/scanner code?
- Did CLI or TUI gain business logic that belongs in a shared service?
- Any speculative trait, generic framework or dependency without a concrete need?

## 7. Update durable documentation

- ADRs for decisions with significant implications; `docs/` for durable product
  or architecture truth. Mark superseded documents; do not quietly rewrite
  history.

## 8. Commit atomically

- Conventional Commit, en-GB, single logical change. Never commit `.specs/`,
  secrets or unverified work.
