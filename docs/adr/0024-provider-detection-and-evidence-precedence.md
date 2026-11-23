# 0024 — Provider detection and evidence precedence

**Status:** Accepted

## Context

The previous detector used a strict first-match cascade (`CDN -> CNAME -> ASN -> header`), substring `contains()` matching on CNAMEs (causing `notgoogle.com` to match `google.com`), and generic `Server` headers (`Microsoft-IIS`) to attribute Azure hosting.

## Decision

1. **Collect all signals first**: `HostingDetector::attribute()` evaluates CDN, CNAME, NS, ASN, and platform headers without early-return short-circuiting.
2. **Label-boundary CNAME matching**: `Provider::matches_cname()` matches exact hostnames, `.suffix` boundaries, or explicit `-` label prefixes (`awsdns-`), never arbitrary substrings.
3. **No generic server software attribution**: `Microsoft-IIS`, `Microsoft-HTTPAPI`, `nginx`, and `Apache` are excluded from `providers.json` and never produce cloud provider attribution.
4. **Explicit conflicts**: Multiple disagreeing non-CDN origin candidates yield `OriginAttribution::Conflicting`.

## Consequences and trade-offs

- Lower false-positive rate at the cost of returning `Unknown` when only generic server headers are observed.

## Verification criteria

- `test_cname_respects_label_boundaries`, `test_kb_integrity_no_duplicate_ids_or_generic_headers`, and `test_iis_and_nginx_headers_do_not_attribute_cloud_provider` pass.
