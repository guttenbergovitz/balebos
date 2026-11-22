---
name: balebos-architecture-review
description: >-
  Audit a Balebos change or module for boundary violations, domain model
  quality, unjustified abstractions and product-scope drift. Produces specific
  actionable findings with file and line references.
---

# Balebos Architecture Review

Read the actual code; never review from memory. Output a numbered list of
findings, each with `path:line`, severity (blocking / should-fix / nit), and a
concrete suggested action. "No findings" is a valid result for a dimension.

## Checklist

1. **Dependency direction.** `domain` must not import CLI, TUI, storage
   implementations or scanner processes. Adapters (`discovery`, `inspection`)
   produce domain observations; nothing domain-level knows Parquet/SQLite/JSONL
   specifics. Flag any violation with `path:line`.

2. **Module responsibilities.** Each module has one clear job per
   `docs/architecture.md`. Flag business logic inside `tui`, SQL/Parquet
   details outside `storage`, probe orchestration outside adapters.

3. **Domain model quality.** Newtyped IDs at boundaries? Enums instead of
   loosely related booleans? Validated constructors? Any primitive obsession or
   stringly-typed state machine?

4. **Ownership and borrowing.** Gratuitous `.clone()`/`.to_owned()`? `Arc<Mutex<_>>`
   where simple ownership works? Unnecessary heap allocation or dynamic dispatch?

5. **Error handling.** Typed errors where callers decide structurally? Any
   `.unwrap()`/`.expect()` without a provable invariant? Silent suppression or
   parse-failure-to-default mapping (blocking).

6. **Unnecessary abstractions.** Traits with one implementation and no testing
   boundary? Generic frameworks? Premature parameterisation? Cite each.

7. **Dependency justification.** Every new crate: purpose, maintenance status,
   licence, compile cost. Standard-library alternatives considered?

8. **Duplication with Angry Turnip.** Any SCA/CVE/secret/SBOM/licence/Dockerfile
   auditing functionality creeping in? Flag as blocking (ADR 0002).

9. **Product-scope drift.** CMDB authority, org hierarchies, approval
   workflows, distributed orchestration, event buses? Flag against ADR 0001.

10. **Documentation quality.** Public items documented? Do docs describe what
    the code actually does? Any placeholder documentation presented as progress?

## Output format

```text
Dimension: Dependency direction
- [blocking] src/domain/model.rs:42 imports crate::storage::parquet — domain
  must not know storage engines. Move the conversion into the storage adapter.
...
Verdict: PASS / PASS WITH NITS / CHANGES REQUIRED (blocking findings present)
```
