# 0023 — Infrastructure attribution taxonomy

**Status:** Accepted

## Context

Representing hosting as a single `provider_name` + `Confidence` conflated distinct technical and organisational roles: domain registrar, authoritative DNS provider, CDN/edge intermediary, network operator (ASN), application platform (PaaS), and origin hosting provider. For example, detecting CloudFront produced `provider_name = "Amazon Web Services", confidence = Confirmed`, falsely claiming knowledge of the origin server.

## Decision

Model `InfrastructureAttribution` (`src/hosting/detector.rs`) with explicit, independent roles:
1. `registrar: Option<RegistrarAttribution>` (accredited registrar from RDAP/WHOIS).
2. `authoritative_dns: Vec<ProviderAttribution>` (from NS delegation).
3. `cdn: Vec<ProviderAttribution>` (observable edge intermediary).
4. `network: Vec<NetworkAttribution>` (observed IP/ASN operator).
5. `application_platform: Vec<ProviderAttribution>` (PaaS/edge platform).
6. `origin: OriginAttribution` (`Observed`, `Conflicting`, or `Unknown`).

When a CDN/edge intermediary is observed, `origin` defaults to `Unknown { reason: "Origin not externally observable behind CDN" }`.

## Consequences and trade-offs

- Eliminates false origin attribution on CloudFront, Cloudflare, Fastly, and Akamai.
- Legacy `HostingObservation` remains readable and backwards-compatible via projection.

## Verification criteria

- `test_detect_cloudfront_masks_origin` and `test_cloudflare_plus_aws_network_separates_roles` pass.
