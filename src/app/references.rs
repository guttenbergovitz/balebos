//! Bounded application & infrastructure reference discovery (ADR 0028).
//!
//! Extracts typed, non-authoritative references from:
//! - Public JavaScript assets (URLs, API bases, GraphQL, WebSocket, OAuth/OIDC, storage)
//! - OpenAPI 3.x / Swagger 2.0 specifications (declared servers & paths)
//! - DNS SRV & structured TXT records (ignoring opaque verification tokens)
//! - urlscan.io indexed browser/resource results
//! - Authorised public repository configurations (Terraform / manifests)
//!
//! A reference is never treated as proof of live runtime communication or ownership.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Deserialize;

use crate::domain::normalize::normalize_fqdn;
use crate::domain::relationship::{EntityKind, RelationshipKind};
use crate::domain::service::{ApplicationReferenceObservation, VerificationStatus};
use crate::error::{BalebosError, Result};
use crate::inventory::ItemType;

const MAX_STATIC_SCAN_BYTES: usize = 512 * 1024;
const MAX_MATCHES_PER_ASSET: usize = 200;

fn is_in_scope(host: &str, approved_scope: &[String]) -> bool {
    if approved_scope.is_empty() {
        return false;
    }
    let fqdn = normalize_fqdn(host);
    approved_scope.iter().any(|s| {
        let sc = normalize_fqdn(s);
        fqdn == sc || fqdn.ends_with(&format!(".{}", sc))
    })
}

/// Redact query-string credential tokens (`?token=...`, `?key=...`, etc.) so
/// extracted evidence never persists secrets unnecessarily.
pub fn redact_url_secrets(raw_url: &str) -> String {
    let secret_re = Regex::new(
        r"(?i)([?&](?:token|key|api_key|apikey|secret|password|auth|access_token|signature)=)[^&\s]+",
    )
    .expect("valid secret redaction regex");
    secret_re
        .replace_all(raw_url, "${1}[REDACTED]")
        .into_owned()
}

/// Classify cloud object storage URLs without implying bucket ownership or
/// directory-listing permissions (ADR 0028).
pub fn classify_storage_url(raw_url: &str) -> Option<(&'static str, String)> {
    let lower = raw_url.trim().to_ascii_lowercase();
    let without_scheme = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .or_else(|| lower.strip_prefix("s3://"))
        .unwrap_or(&lower);
    let host_part = without_scheme.split('/').next().unwrap_or("");
    let host = host_part.split(':').next().unwrap_or("");

    if host.ends_with(".s3.amazonaws.com")
        || host == "s3.amazonaws.com"
        || lower.starts_with("s3://")
    {
        let canonical = format!(
            "s3://{}",
            without_scheme.split('?').next().unwrap_or(without_scheme)
        );
        return Some(("storage_s3", canonical));
    }
    if host == "storage.googleapis.com" || host.ends_with(".storage.googleapis.com") {
        let canonical = format!(
            "gcs://{}",
            without_scheme.split('?').next().unwrap_or(without_scheme)
        );
        return Some(("storage_gcs", canonical));
    }
    if host.ends_with(".blob.core.windows.net") {
        let canonical = format!(
            "azure-blob://{}",
            without_scheme.split('?').next().unwrap_or(without_scheme)
        );
        return Some(("storage_azure", canonical));
    }
    None
}

fn host_from_url(raw_url: &str) -> Option<String> {
    let lower = raw_url.trim().to_ascii_lowercase();
    let after_scheme = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .or_else(|| lower.strip_prefix("wss://"))
        .or_else(|| lower.strip_prefix("ws://"))?;
    let authority = after_scheme.split(['/', '?', '#']).next()?;
    let host = authority.split('@').next_back()?.split(':').next()?;
    let fqdn = normalize_fqdn(host);
    if fqdn.is_empty() || !fqdn.contains('.') {
        None
    } else {
        Some(fqdn)
    }
}

fn has_dev_or_placeholder_hint(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "localhost",
        "127.0.0.1",
        "staging",
        ".dev.",
        "-dev.",
        "dev-",
        "sandbox",
        "example.invalid",
        "placeholder",
    ]
    .iter()
    .any(|kw| lower.contains(kw))
}

fn has_deprecated_hint(context: &str) -> bool {
    let lower = context.to_ascii_lowercase();
    [
        "deprecated",
        "legacy",
        "sunset",
        "old-api",
        "old_api",
        "/v1-deprecated",
    ]
    .iter()
    .any(|kw| lower.contains(kw))
}

/// Bounded static reference extraction from a public JavaScript bundle.
/// Never executes untrusted scripts.
pub fn extract_js_references(
    origin_asset: &str,
    discovered_in_url: &str,
    js_content: &str,
    approved_scope: &[String],
    source_recorded_at: Option<DateTime<Utc>>,
    ingested_at: DateTime<Utc>,
) -> Vec<ApplicationReferenceObservation> {
    let bounded = if js_content.len() > MAX_STATIC_SCAN_BYTES {
        &js_content[..MAX_STATIC_SCAN_BYTES]
    } else {
        js_content
    };
    let origin = normalize_fqdn(origin_asset);
    let url_re =
        Regex::new(r#"(?i)(?:https?|wss?)://[a-z0-9.\-_/:%?=&+#~]+"#).expect("valid JS URL regex");

    let mut seen = BTreeSet::new();
    let mut out = Vec::new();

    for m in url_re.find_iter(bounded).take(MAX_MATCHES_PER_ASSET) {
        let raw_url = m.as_str().trim_end_matches(['"', '\'', '`', ',', ';', ')']);
        let safe_url = redact_url_secrets(raw_url);
        let Some(host) = host_from_url(&safe_url) else {
            continue;
        };

        // Context window around match to detect deprecation/dev hints.
        let start = m.start().saturating_sub(48);
        let end = (m.end() + 48).min(bounded.len());
        let ctx = &bounded[start..end];
        let deprecated = has_deprecated_hint(ctx);
        let dev_hint = has_dev_or_placeholder_hint(&safe_url) || has_dev_or_placeholder_hint(ctx);

        let lower_url = safe_url.to_ascii_lowercase();
        let (ref_type, target_kind, rel_kind, target_id, uncertainty) = if let Some((
            stype,
            storage_id,
        )) =
            classify_storage_url(&safe_url)
        {
            (
                    stype.to_string(),
                    EntityKind::StorageEndpoint,
                    RelationshipKind::ReferencesStorage,
                    storage_id,
                    Some("Public storage URL reference only; does not imply bucket ownership or listing permissions".to_string()),
                )
        } else if lower_url.starts_with("ws://") || lower_url.starts_with("wss://") {
            (
                "js_websocket".to_string(),
                EntityKind::ApiService,
                RelationshipKind::ReferencesApi,
                host.clone(),
                Some(
                    "Static WebSocket reference in JavaScript; runtime reachability unverified"
                        .to_string(),
                ),
            )
        } else if lower_url.contains("graphql") {
            (
                "js_graphql".to_string(),
                EntityKind::ApiService,
                RelationshipKind::ReferencesApi,
                host.clone(),
                Some(
                    "Static GraphQL reference in JavaScript; introspection not executed"
                        .to_string(),
                ),
            )
        } else if lower_url.contains("openid-configuration")
            || lower_url.contains("/oauth")
            || host.starts_with("auth.")
            || host.starts_with("idp.")
            || host.starts_with("sso.")
        {
            (
                "js_oauth_authority".to_string(),
                EntityKind::ApplicationInterface,
                RelationshipKind::UsesIdentityProvider,
                host.clone(),
                Some("Referenced identity authority in JavaScript; not verified".to_string()),
            )
        } else if lower_url.contains("/api") || host.starts_with("api.") {
            (
                "js_api_url".to_string(),
                EntityKind::ApiService,
                RelationshipKind::ReferencesApi,
                host.clone(),
                Some(
                    "Static API reference in JavaScript; not verified as active runtime traffic"
                        .to_string(),
                ),
            )
        } else {
            (
                "js_external_url".to_string(),
                EntityKind::Fqdn,
                RelationshipKind::LoadsResourceFrom,
                host.clone(),
                Some("Static URL reference in JavaScript".to_string()),
            )
        };

        if !seen.insert((ref_type.clone(), target_id.clone())) {
            continue;
        }

        out.push(ApplicationReferenceObservation {
            item_type: ItemType::ApplicationReference,
            id: format!("ref-js-{}-{}", origin, seen.len()),
            origin_asset: origin.clone(),
            referenced_target: target_id,
            target_kind,
            relationship_kind: rel_kind,
            reference_type: ref_type,
            discovered_in: redact_url_secrets(discovered_in_url),
            source: "js_static".to_string(),
            source_recorded_at,
            ingested_at,
            verification: VerificationStatus::Referenced,
            in_scope: is_in_scope(&host, approved_scope),
            deprecated_hint: deprecated,
            dev_or_placeholder_hint: dev_hint,
            evidence: vec![format!(
                "Matched URL {} in {}",
                safe_url,
                redact_url_secrets(discovered_in_url)
            )],
            uncertainty,
        });
    }

    out
}

#[derive(Debug, Deserialize)]
struct OpenApiDoc {
    #[serde(default)]
    servers: Vec<OpenApiServer>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default, rename = "basePath")]
    base_path: Option<String>,
    #[serde(default)]
    schemes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OpenApiServer {
    url: String,
    #[serde(default)]
    description: Option<String>,
}

/// Extract declared API server endpoints from an OpenAPI 3.x or Swagger 2.0
/// JSON document without executing any API operations (ADR 0028).
pub fn parse_openapi_references(
    origin_asset: &str,
    doc_url: &str,
    doc_json: &str,
    approved_scope: &[String],
    source_recorded_at: Option<DateTime<Utc>>,
    ingested_at: DateTime<Utc>,
) -> Result<Vec<ApplicationReferenceObservation>> {
    let doc: OpenApiDoc = serde_json::from_str(doc_json)
        .map_err(|e| BalebosError::Validation(format!("Invalid OpenAPI/Swagger JSON: {}", e)))?;
    let origin = normalize_fqdn(origin_asset);
    let mut server_entries: Vec<(String, Option<String>)> = doc
        .servers
        .into_iter()
        .map(|s| (s.url, s.description))
        .collect();

    if let Some(h) = doc.host {
        let scheme = doc.schemes.first().map(|s| s.as_str()).unwrap_or("https");
        let base = doc.base_path.unwrap_or_default();
        server_entries.push((format!("{}://{}{}", scheme, h, base), None));
    }

    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (url, desc) in server_entries {
        let safe_url = redact_url_secrets(&url);
        let Some(host) = host_from_url(&safe_url) else {
            continue;
        };
        if !seen.insert(host.clone()) {
            continue;
        }
        let dev_hint = has_dev_or_placeholder_hint(&safe_url)
            || desc.as_deref().is_some_and(has_dev_or_placeholder_hint);
        out.push(ApplicationReferenceObservation {
            item_type: ItemType::ApplicationReference,
            id: format!("ref-openapi-{}-{}", origin, seen.len()),
            origin_asset: origin.clone(),
            referenced_target: host.clone(),
            target_kind: EntityKind::ApiService,
            relationship_kind: RelationshipKind::DeclaresService,
            reference_type: "openapi_server".to_string(),
            discovered_in: redact_url_secrets(doc_url),
            source: "openapi".to_string(),
            source_recorded_at,
            ingested_at,
            verification: VerificationStatus::Declared,
            in_scope: is_in_scope(&host, approved_scope),
            deprecated_hint: desc.as_deref().is_some_and(has_deprecated_hint),
            dev_or_placeholder_hint: dev_hint,
            evidence: vec![format!(
                "OpenAPI server declaration: {}{}",
                safe_url,
                desc.map(|d| format!(" ({})", d)).unwrap_or_default()
            )],
            uncertainty: Some(
                "Declared in OpenAPI document; reachability and runtime usage unverified"
                    .to_string(),
            ),
        });
    }
    Ok(out)
}

fn is_opaque_verification_txt(txt: &str) -> bool {
    let lower = txt.trim().to_ascii_lowercase();
    [
        "google-site-verification=",
        "ms=",
        "apple-domain-verification=",
        "facebook-domain-verification=",
        "atlassian-domain-verification=",
        "docusign=",
        "adobe-idp-site-verification=",
        "globalsign-domain-verification=",
        "stripe-verification=",
        "zoom_verify_",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

/// Extract typed infrastructure declarations from DNS SRV and structured TXT
/// records, strictly ignoring opaque domain-verification tokens (ADR 0028).
pub fn extract_dns_srv_txt_references(
    origin_asset: &str,
    srv_records: &[String],
    txt_records: &[String],
    approved_scope: &[String],
    observed_at: DateTime<Utc>,
) -> Vec<ApplicationReferenceObservation> {
    let origin = normalize_fqdn(origin_asset);
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();

    for srv in srv_records {
        // Accept `"priority weight port target"` or `"_svc._proto.domain -> target:port"`.
        let tokens: Vec<&str> = srv.split_whitespace().collect();
        let (target_raw, port_opt) = if tokens.len() >= 4 {
            (tokens[3], tokens[2].parse::<u16>().ok())
        } else if let Some((_lhs, rhs)) = srv.split_once("->") {
            let r = rhs.trim();
            if let Some((h, p)) = r.rsplit_once(':') {
                (h.trim(), p.trim().parse::<u16>().ok())
            } else {
                (r, None)
            }
        } else {
            continue;
        };

        let target_host = normalize_fqdn(target_raw);
        if target_host.is_empty() || target_host == "." {
            continue;
        }
        let target_id = match port_opt {
            Some(p) => format!("{}:{}/tcp", target_host, p),
            None => target_host.clone(),
        };
        if seen.insert(("dns_srv".to_string(), target_id.clone())) {
            out.push(ApplicationReferenceObservation {
                item_type: ItemType::ApplicationReference,
                id: format!("ref-srv-{}-{}", origin, seen.len()),
                origin_asset: origin.clone(),
                referenced_target: target_id,
                target_kind: if port_opt.is_some() {
                    EntityKind::NetworkService
                } else {
                    EntityKind::Fqdn
                },
                relationship_kind: RelationshipKind::DeclaresService,
                reference_type: "dns_srv".to_string(),
                discovered_in: format!("DNS SRV on {}", origin),
                source: "dns_srv_txt".to_string(),
                source_recorded_at: Some(observed_at),
                ingested_at: observed_at,
                verification: VerificationStatus::Declared,
                in_scope: is_in_scope(&target_host, approved_scope),
                deprecated_hint: false,
                dev_or_placeholder_hint: false,
                evidence: vec![format!("SRV record: {}", srv)],
                uncertainty: Some(
                    "DNS SRV configuration declaration; does not prove service is currently reachable".to_string(),
                ),
            });
        }
    }

    for txt in txt_records {
        let trimmed = txt.trim().trim_matches('"');
        if is_opaque_verification_txt(trimmed) {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("v=spf1") {
            for part in lower.split_whitespace() {
                let inc = part
                    .strip_prefix("include:")
                    .or_else(|| part.strip_prefix("redirect="));
                if let Some(domain) = inc {
                    let target = normalize_fqdn(domain);
                    if !target.is_empty()
                        && seen.insert(("dns_spf_include".to_string(), target.clone()))
                    {
                        out.push(ApplicationReferenceObservation {
                            item_type: ItemType::ApplicationReference,
                            id: format!("ref-txt-{}-{}", origin, seen.len()),
                            origin_asset: origin.clone(),
                            referenced_target: target.clone(),
                            target_kind: EntityKind::Domain,
                            relationship_kind: RelationshipKind::DeclaresService,
                            reference_type: "dns_spf_include".to_string(),
                            discovered_in: format!("DNS TXT SPF on {}", origin),
                            source: "dns_srv_txt".to_string(),
                            source_recorded_at: Some(observed_at),
                            ingested_at: observed_at,
                            verification: VerificationStatus::Declared,
                            in_scope: is_in_scope(&target, approved_scope),
                            deprecated_hint: false,
                            dev_or_placeholder_hint: false,
                            evidence: vec![format!("SPF include: {}", domain)],
                            uncertainty: Some("SPF mail-sender policy declaration".to_string()),
                        });
                    }
                }
            }
        } else if lower.starts_with("v=dmarc1") {
            for part in lower.split(';') {
                let part = part.trim();
                if let Some(uri_list) = part
                    .strip_prefix("rua=")
                    .or_else(|| part.strip_prefix("ruf="))
                {
                    for uri in uri_list.split(',') {
                        if let Some((_, domain_part)) = uri.trim().split_once('@') {
                            let target = normalize_fqdn(domain_part);
                            if !target.is_empty()
                                && seen.insert(("dns_dmarc_rua".to_string(), target.clone()))
                            {
                                out.push(ApplicationReferenceObservation {
                                    item_type: ItemType::ApplicationReference,
                                    id: format!("ref-txt-{}-{}", origin, seen.len()),
                                    origin_asset: origin.clone(),
                                    referenced_target: target.clone(),
                                    target_kind: EntityKind::Domain,
                                    relationship_kind: RelationshipKind::DeclaresService,
                                    reference_type: "dns_dmarc_rua".to_string(),
                                    discovered_in: format!("DNS TXT DMARC on {}", origin),
                                    source: "dns_srv_txt".to_string(),
                                    source_recorded_at: Some(observed_at),
                                    ingested_at: observed_at,
                                    verification: VerificationStatus::Declared,
                                    in_scope: is_in_scope(&target, approved_scope),
                                    deprecated_hint: false,
                                    dev_or_placeholder_hint: false,
                                    evidence: vec![format!(
                                        "DMARC reporting destination: {}",
                                        target
                                    )],
                                    uncertainty: Some(
                                        "DMARC reporting target declaration".to_string(),
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    out
}

#[derive(Debug, Deserialize)]
struct UrlscanResult {
    #[serde(default)]
    task: Option<UrlscanTask>,
    #[serde(default)]
    lists: Option<UrlscanLists>,
}

#[derive(Debug, Deserialize)]
struct UrlscanTask {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    time: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct UrlscanLists {
    #[serde(default)]
    urls: Vec<String>,
    #[serde(default)]
    domains: Vec<String>,
}

/// Parse urlscan.io result JSON while keeping indexed results strictly separate
/// from newly requested scans (ADR 0027 / ADR 0031).
pub fn parse_urlscan_result_json(
    origin_asset: &str,
    json: &str,
    is_newly_submitted_scan: bool,
    approved_scope: &[String],
    ingested_at: DateTime<Utc>,
) -> Result<Vec<ApplicationReferenceObservation>> {
    let parsed: UrlscanResult = serde_json::from_str(json)
        .map_err(|e| BalebosError::Validation(format!("Invalid urlscan.io JSON: {}", e)))?;
    let origin = normalize_fqdn(origin_asset);
    let recorded_at = parsed.task.as_ref().and_then(|t| t.time);
    let scan_url = parsed
        .task
        .as_ref()
        .and_then(|t| t.url.clone())
        .unwrap_or_else(|| format!("https://{}", origin));
    let source = if is_newly_submitted_scan {
        "urlscan_io_submitted"
    } else {
        "urlscan_io_indexed"
    };

    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    if let Some(lists) = parsed.lists {
        for raw_url in lists.urls {
            let safe_url = redact_url_secrets(&raw_url);
            if let Some((stype, storage_id)) = classify_storage_url(&safe_url) {
                if seen.insert((stype.to_string(), storage_id.clone())) {
                    out.push(ApplicationReferenceObservation {
                        item_type: ItemType::ApplicationReference,
                        id: format!("ref-urlscan-{}-{}", origin, seen.len()),
                        origin_asset: origin.clone(),
                        referenced_target: storage_id,
                        target_kind: EntityKind::StorageEndpoint,
                        relationship_kind: RelationshipKind::ReferencesStorage,
                        reference_type: stype.to_string(),
                        discovered_in: redact_url_secrets(&scan_url),
                        source: source.to_string(),
                        source_recorded_at: recorded_at,
                        ingested_at,
                        verification: VerificationStatus::Indexed,
                        in_scope: false,
                        deprecated_hint: false,
                        dev_or_placeholder_hint: false,
                        evidence: vec![format!("urlscan.io observed request to {}", safe_url)],
                        uncertainty: Some(
                            "Observed in urlscan.io; does not prove backend dependency or bucket ownership".to_string(),
                        ),
                    });
                }
            }
        }
        for dom in lists.domains {
            let fqdn = normalize_fqdn(&dom);
            if fqdn.is_empty() || fqdn == origin {
                continue;
            }
            if seen.insert(("urlscan_request".to_string(), fqdn.clone())) {
                out.push(ApplicationReferenceObservation {
                    item_type: ItemType::ApplicationReference,
                    id: format!("ref-urlscan-{}-{}", origin, seen.len()),
                    origin_asset: origin.clone(),
                    referenced_target: fqdn.clone(),
                    target_kind: EntityKind::Fqdn,
                    relationship_kind: RelationshipKind::LoadsResourceFrom,
                    reference_type: "urlscan_request".to_string(),
                    discovered_in: redact_url_secrets(&scan_url),
                    source: source.to_string(),
                    source_recorded_at: recorded_at,
                    ingested_at,
                    verification: VerificationStatus::Indexed,
                    in_scope: is_in_scope(&fqdn, approved_scope),
                    deprecated_hint: false,
                    dev_or_placeholder_hint: false,
                    evidence: vec![format!("urlscan.io observed browser request domain {}", fqdn)],
                    uncertainty: Some(
                        "Browser resource request recorded by urlscan.io; not proof of backend dependency".to_string(),
                    ),
                });
            }
        }
    }
    Ok(out)
}

/// Extract declared infrastructure references from an authorised public
/// repository configuration file (Terraform, Kubernetes/Helm, env template).
/// Never exposes secret values and always marks results `Declared`.
pub fn parse_public_repo_references(
    origin_asset: &str,
    repo_path: &str,
    content: &str,
    approved_scope: &[String],
    source_recorded_at: Option<DateTime<Utc>>,
    ingested_at: DateTime<Utc>,
) -> Vec<ApplicationReferenceObservation> {
    let bounded = if content.len() > MAX_STATIC_SCAN_BYTES {
        &content[..MAX_STATIC_SCAN_BYTES]
    } else {
        content
    };
    let origin = normalize_fqdn(origin_asset);
    let url_re = Regex::new(r#"(?i)(?:https?|wss?|s3)://[a-z0-9.\-_/:%?=&+#~]+"#)
        .expect("valid repo URL regex");

    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for m in url_re.find_iter(bounded).take(MAX_MATCHES_PER_ASSET) {
        let raw_url = m.as_str().trim_end_matches(['"', '\'', '`', ',', ';', ')']);
        let safe_url = redact_url_secrets(raw_url);
        let dev_hint =
            has_dev_or_placeholder_hint(&safe_url) || has_dev_or_placeholder_hint(repo_path);

        if let Some((stype, storage_id)) = classify_storage_url(&safe_url) {
            if seen.insert((stype.to_string(), storage_id.clone())) {
                out.push(ApplicationReferenceObservation {
                    item_type: ItemType::ApplicationReference,
                    id: format!("ref-repo-{}-{}", origin, seen.len()),
                    origin_asset: origin.clone(),
                    referenced_target: storage_id,
                    target_kind: EntityKind::StorageEndpoint,
                    relationship_kind: RelationshipKind::ReferencesStorage,
                    reference_type: format!("repo_{}", stype),
                    discovered_in: repo_path.to_string(),
                    source: "public_repo".to_string(),
                    source_recorded_at,
                    ingested_at,
                    verification: VerificationStatus::Declared,
                    in_scope: false,
                    deprecated_hint: false,
                    dev_or_placeholder_hint: dev_hint,
                    evidence: vec![format!("Declared storage URL in {}", repo_path)],
                    uncertainty: Some(
                        "Repository configuration declaration; may be historical, dev-only, or undeployed".to_string(),
                    ),
                });
            }
            continue;
        }

        let Some(host) = host_from_url(&safe_url) else {
            continue;
        };
        if seen.insert(("repo_config".to_string(), host.clone())) {
            out.push(ApplicationReferenceObservation {
                item_type: ItemType::ApplicationReference,
                id: format!("ref-repo-{}-{}", origin, seen.len()),
                origin_asset: origin.clone(),
                referenced_target: host.clone(),
                target_kind: EntityKind::ApiService,
                relationship_kind: RelationshipKind::DeclaresService,
                reference_type: "repo_config".to_string(),
                discovered_in: repo_path.to_string(),
                source: "public_repo".to_string(),
                source_recorded_at,
                ingested_at,
                verification: VerificationStatus::Declared,
                in_scope: is_in_scope(&host, approved_scope),
                deprecated_hint: false,
                dev_or_placeholder_hint: dev_hint,
                evidence: vec![format!("Declared endpoint {} in {}", safe_url, repo_path)],
                uncertainty: Some(
                    "Repository configuration declaration; may be historical, dev-only, or undeployed".to_string(),
                ),
            });
        }
    }
    out
}
