---
name: balebos-security-audit
description: >-
  Security review specific to an infrastructure discovery tool: inspection
  scope, subprocess execution, DNS/redirect handling, resource bounds and
  evidence integrity. Produces specific actionable findings with file and line
  references.
---

# Balebos Security Audit

Balebos actively contacts discovered infrastructure and executes external
binaries. Review the actual code and output numbered findings with `path:line`,
severity (blocking / should-fix / nit) and a concrete fix. This skill is
specific to discovery tooling — it is not a generic web-app checklist.

## Checklist

1. **Input validation.** Operator-supplied domains and file paths validated
   before use? Malformed targets rejected at the boundary, not deep in probes?

2. **Inspection scope.** Is scope explicitly authorised and bounded? Are
   private/link-local/loopback addresses from untrusted discovery results
   filtered unless an explicit internal-network mode is on? Flag any path where
   a discovered string becomes a probed target without scope checks.

3. **DNS resolution and redirects.** Do redirect chains cross scope boundaries
   without notice (e.g. external redirect introduced)? Is redirect validation
   consistent with the selected scope?

4. **Network boundary enforcement.** Request limits, meaningful timeouts,
   no unbounded fan-out per target, no retry storms. Concurrency caps enforced
   programmatically, not by operator discipline.

5. **Subprocess execution.** `std::process::Command` with explicit args only —
   no shell-string construction anywhere. Tool identity and version captured?
   Exit status and stderr handled? Bound stdout/stderr (no unbounded capture)?
   Termination and cancellation defined: timed-out children killed and reaped,
   descendants where supported, no orphaned scanners?

6. **Filesystem paths and traversal.** Storage dir, export paths and import
   paths operator-controlled but never built from probe results. No traversal
   via asset names into export filenames.

7. **Secrets in logs.** Tokens/keys in subprocess arguments, env or captured
   output ever printed, stored or exported? (Balebos should handle none, but
   verify.)

8. **Resource exhaustion.** Unbounded buffers, unbounded record loading,
   memory growth per batch, file descriptor leaks on error paths, zombie
   processes.

9. **Malformed external tool output.** Non-JSON lines, huge lines, unexpected
   schema, tool version drift — handled without panic, without silent drop of
   evidence, and with the failure recorded as an inspection outcome (ADR 0005).

10. **Data and report integrity.** Export paths atomic? Findings link to real
    observations? No way for a scanned target to inject content into reports
    unescaped (CSV formula injection, HTML/JSON injection into exports)?

## Output format

```text
Dimension: Subprocess execution
- [blocking] src/app/probe/asn.rs:51 shells out to gtimeout/timeout binaries —
  ad hoc, platform-dependent, no reaping guarantee. Use a uniform timeout +
  kill + reap wrapper for all adapters.
...
Verdict: PASS / PASS WITH NITS / CHANGES REQUIRED (blocking findings present)
```
