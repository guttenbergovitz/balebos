# 0015 — Hash chain and external anchoring (future)

**Status:** Proposed

## Decision (draft — deferred)

Optional enhancement over ADR 0014: chain sealed segments by including the
previous segment's SHA-256 in each manifest, forming a local hash chain.
Encoding rules must be unambiguous (hash the previous segment's file digest,
not its manifest; canonical byte encoding documented in the manifest
schema).

If tamper evidence beyond local detection is required, periodically anchor
the chain head (a signed manifest or the head hash) **outside the local
trust boundary** (e.g. a remote attestation service, signed tag, or WORM
store). No blockchain, no distributed consensus, no elaborate cryptography.

## Context

A local chain reveals inconsistencies to an honest verifier but is
reconstructable by an attacker who controls the journal and its only hash
reference. The three properties must stay distinct: durability (SQLite
transactions, crash-safe publication), integrity (SHA-256 against a trusted
digest), immutability (only an external WORM policy — ADR 0016 — can
approach it, under an explicit threat model).

## Evidence required to proceed

1. A concrete verifier workflow that the chain improves upon plain segment
   hashing.
2. Chosen anchoring target and its trust properties.
3. Manifest schema version that reserves the chain fields.
