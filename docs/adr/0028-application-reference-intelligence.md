# 0028 — Application reference intelligence

**Status:** Accepted (bounded offline/static extractors & relationship projection)

## Context

Public JavaScript bundles, OpenAPI/Swagger specifications, DNS SRV/TXT records, cloud storage URLs, and public repository configurations reveal APIs, identity providers, message/service endpoints, and external dependencies that DNS A/AAAA lookups alone miss.

## Decision

1. **Shared domain representation (`ApplicationReferenceObservation`)**: All reference sources emit typed observations feeding the same relationship model (`loads_resource_from`, `references_api`, `uses_identity_provider`, `references_storage`, `declares_service`) with provenance, context (`discovered_in`), scope classification, and verification status (`Referenced` / `Declared`, never `Observed` runtime traffic).
2. **Bounded static extraction**:
   - **JavaScript assets**: Extract API base URLs, GraphQL/WebSocket endpoints, OAuth/OIDC authorities, and storage URLs without executing untrusted scripts; redact secret-like tokens in stored context.
   - **OpenAPI / Swagger**: Parse declared `servers`, base paths, and security scheme types without executing API operations.
   - **DNS SRV & structured TXT**: Parse SRV targets/ports, SPF `include:`/`redirect=`, DMARC `rua`/`ruf`, and recognised SaaS hints; ignore opaque domain verification tokens (`google-site-verification=`, `MS=`, random tokens).
   - **Public storage URLs**: Classify S3, GCS, and Azure Blob references as `StorageEndpoint` candidates without inferring ownership or bucket-listing permissions.
   - **Public repository config**: Treat Terraform/manifest/config references as `Declared` / `Referenced` only (may be historical or dev-only).

## Alternatives considered

- **Headless browser execution or JS AST evaluation**: Rejected as unbounded and unsafe for untrusted public scripts in core discovery.
- **Treating all TXT records as relationships**: Rejected because verification tokens create noise and leak validation markers.

## Consequences and risks

- **Consequences**: Balebos discovers APIs, storage endpoints, and service dependencies from static artefacts while preserving the gap between declaration and live confirmation.
- **Risks**: Regex/static extraction can surface deprecated or placeholder URLs; mitigated by keeping verification status `Referenced`/`Declared` and excluding them from automatic active probing when out of scope.

## Verification criteria

- Benchmark fixtures verify JS API/deprecated API extraction, multi-server OpenAPI parsing, SRV parsing, TXT verification token suppression, S3 URL non-ownership/non-listing semantics, and dev-only repository config handling.
