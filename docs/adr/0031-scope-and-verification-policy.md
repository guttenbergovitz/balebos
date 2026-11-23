# 0031 — Scope and verification policy

**Status:** Accepted

## Context

Discovering an out-of-scope IP, port, API URL, or storage bucket via passive sources (CT, Censys, Shodan, urlscan.io, JS, OpenAPI, public repositories) must never trigger unsolicited active probing or third-party scanning.

## Decision

1. **Architectural separation**: Every discovery source and workflow declares `OperationMode::Passive` or `OperationMode::Active`.
2. **Passive discovery never grants active scope**: Candidates surfaced from passive indexes or static references remain in `VerificationStatus::Unverified` / `ScopeStatus::OutOfScope` unless the target is explicitly within an operator-authorised scope (`ScopePolicy`).
3. **Active observation budgets**: Active operations enforce explicit target authorisation, timeouts, concurrency/rate limits, request/connection caps, cancellation, and internal/loopback/link-local protection (`src/app/probe/scope.rs`).
4. **Prohibited behaviours**: No automatic authentication, no protocol fuzzing, no unrestricted port scanning, and no automatic submission of sensitive/internal URLs to external scanners such as urlscan.io.

## Alternatives considered

- **Auto-probing every hostname or IP discovered in JS/OpenAPI/Censys**: Rejected as unsafe and out of scope.

## Consequences and risks

- **Consequences**: Out-of-scope discoveries remain inspectable, explainable, and reviewable without generating unauthorised network traffic.

## Verification criteria

- Tests prove that out-of-scope candidates from Censys/JS/OpenAPI/repo are surfaced passively with `active_probing_authorised == false` and are skipped by active verification gates.
