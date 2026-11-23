# 0025 — Authoritative registrar identity and RDAP/WHOIS parsing

**Status:** Accepted

## Context

The previous registration probe queried subdomains directly, ignored RDAP `publicIds` (IANA Registrar ID) and `handle`, omitted completeness status, and split WHOIS lines on `:` with `.nth(1)`—truncating RFC3339 timestamps at the hour colon.

## Decision

1. **Registrable domain resolution**: `registrable_domain()` (`src/domain/normalize.rs`) resolves subdomains to their eTLD+1 using a two-part public-suffix table before querying RDAP/WHOIS.
2. **Authoritative RDAP fields**: `parse_rdap_json()` extracts `iana_registrar_id` from `publicIds` (`IANA Registrar ID`), `registrar_handle`, and sets `completeness` (`complete` vs `missing_registrar`).
3. **WHOIS fallback**: `parse_whois_text()` uses `split_once(':')` and preserves unparseable dates in `raw_dates` without fabricating timestamps.

## Consequences and trade-offs

- ccTLDs without IANA Registrar IDs remain explicitly supported with `iana_registrar_id = None` and `completeness` recorded.

## Verification criteria

- `registrable_domain_handles_subdomains_and_two_part_tlds`, `rdap_parser_extracts_nested_registrar_and_iana_id`, `rdap_parser_flags_missing_registrar_completeness`, and `whois_parser_handles_rfc3339_colons_and_iana_id` pass.
