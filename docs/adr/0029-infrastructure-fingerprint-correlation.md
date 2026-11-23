# 0029 — Infrastructure fingerprint correlation

**Status:** Accepted

## Context

Correlating endpoints requires distinguishing discriminating cryptographic or application-specific fingerprints (such as uncommon SSH host public keys or specific management interface signatures) from generic banners (`nginx`, generic login pages) and multi-tenant TLS certificates.

## Decision

1. **Structured fingerprints (`EndpointFingerprint`)**: Represent TLS SHA-256/SPKI/SAN, SSH host public-key fingerprints, HTTP favicon hashes, and management interface signatures as typed evidence with explicit `FingerprintDiscriminatingPower` (`Discriminating`, `Moderate`, `Generic`).
2. **Explicit relationship semantics**:
   - Matching uncommon SSH host keys across two endpoints projects `shares_ssh_host_key` (`StrongCandidate`), still without automatic ownership attribution.
   - Generic web servers or generic login pages do not promote management interface or shared-infrastructure claims.
   - Shared TLS certificates on CDN/multi-tenant hosts remain `WeakSharedClue` (ADR 0021).
3. **Management interface separation**: Software identity, authentication requirement, and reachability are recorded as separate facts; Balebos never classifies a management interface as vulnerable.

## Alternatives considered

- **Numeric similarity scoring across HTTP headers**: Rejected as opaque and prone to false positives on default cloud images.

## Consequences and risks

- **Consequences**: Operators can inspect why two services or endpoints are linked and whether a fingerprint is discriminating or generic.
- **Risks**: Cloned VM templates can share SSH host keys; mitigated by classifying SSH key matches as `StrongCandidate` rather than `OperatorConfirmed` ownership.

## Verification criteria

- Benchmark tests verify management interface detection vs generic login page rejection, SSH host-key correlation, and shared-CDN TLS suppression.
