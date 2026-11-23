# 0016 — Optional remote WORM archival (contract only)

**Status:** Proposed

## Decision (draft — contract, no implementation)

The archive interface permits a future adapter that uploads sealed segments
and manifests to AWS S3 with Object Lock retention and lifecycle transition
to Glacier, providing verification and restoration/rehydration.

Contract requirements recorded now so the archive format does not preclude
it:

- Adapter operates on already-sealed segments + manifests (ADR 0014); it
  never produces or modifies evidence.
- Object Lock (WORM, needs bucket versioning + explicit retention config)
  and Glacier (storage class, retrieval delays/costs) are distinct
  concerns; an adapter must handle both explicitly.
- Small objects are economically inefficient; batching/segment sizing is an
  adapter concern.
- **No AWS SDK in the default binary.** The adapter ships as an optional
  feature or separate tool when implemented.
- AWS is an optional destination, never a persistence foundation; local
  operation never requires it.

## Context

Long-term protected storage is a real future need; coupling the codebase
to a cloud SDK today would violate the dependency policy (ADR 0017) without
a concrete operator. Recording the contract now keeps the door open
cheaply.

## Evidence required to proceed

An explicit operator request with retention requirements and a target
account/bucket configuration.
