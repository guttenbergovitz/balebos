//! Deterministic relationship projections and investigation queries.
//!
//! Projects typed, evidence-backed relationships directly from immutable
//! `Records` without mutating historical data or conflating shared
//! infrastructure with common ownership (ADR 0011).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, Utc};

use crate::domain::normalize::normalize_domain;
use crate::domain::relationship::{
    EntityKind, EntityRef, ObservedRelationship, RelationshipEvidence, RelationshipKind,
    SignalCategory,
};
use crate::storage::reader::Records;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey {
    source_kind: EntityKind,
    source_id: String,
    kind: RelationshipKind,
    target_kind: EntityKind,
    target_id: String,
}

struct EdgeAcc {
    source: EntityRef,
    kind: RelationshipKind,
    target: EntityRef,
    signal: SignalCategory,
    observation_ids: BTreeSet<String>,
    sources: BTreeSet<String>,
    first_seen: DateTime<Utc>,
    last_seen: DateTime<Utc>,
    uncertainty: Option<String>,
}

fn normalise_host(raw: &str) -> String {
    normalize_domain(raw.trim().trim_end_matches('.'))
}

fn host_entity(raw: &str) -> EntityRef {
    let norm = normalise_host(raw);
    let kind = if norm.split('.').count() <= 2 {
        EntityKind::Domain
    } else {
        EntityKind::Fqdn
    };
    EntityRef::new(kind, norm)
}

#[allow(clippy::too_many_arguments)]
fn record_edge(
    acc: &mut BTreeMap<EdgeKey, EdgeAcc>,
    source: EntityRef,
    kind: RelationshipKind,
    target: EntityRef,
    signal: SignalCategory,
    obs_id: &str,
    tool: &str,
    at: DateTime<Utc>,
    uncertainty: Option<String>,
) {
    let key = EdgeKey {
        source_kind: source.kind,
        source_id: source.id.clone(),
        kind,
        target_kind: target.kind,
        target_id: target.id.clone(),
    };
    acc.entry(key)
        .and_modify(|e| {
            e.observation_ids.insert(obs_id.to_string());
            e.sources.insert(tool.to_string());
            if at < e.first_seen {
                e.first_seen = at;
            }
            if at > e.last_seen {
                e.last_seen = at;
            }
            if e.uncertainty.is_none() {
                e.uncertainty = uncertainty.clone();
            }
        })
        .or_insert_with(|| {
            let mut obs_ids = BTreeSet::new();
            obs_ids.insert(obs_id.to_string());
            let mut sources = BTreeSet::new();
            sources.insert(tool.to_string());
            EdgeAcc {
                source,
                kind,
                target,
                signal,
                observation_ids: obs_ids,
                sources,
                first_seen: at,
                last_seen: at,
                uncertainty,
            }
        });
}

/// Project all persisted observations into deterministic, evidence-backed
/// relationships (including historical vs current status).
pub fn project_relationships(records: &Records) -> Vec<ObservedRelationship> {
    let mut acc: BTreeMap<EdgeKey, EdgeAcc> = BTreeMap::new();
    // Track the latest timestamp observed for each (source_id, kind) pair
    // so we can mark edges that are still present in the latest observation.
    let mut latest_at: HashMap<(String, RelationshipKind), DateTime<Utc>> = HashMap::new();

    let mut bump_latest = |src_id: &str, kind: RelationshipKind, at: DateTime<Utc>| {
        latest_at
            .entry((src_id.to_string(), kind))
            .and_modify(|prev| {
                if at > *prev {
                    *prev = at;
                }
            })
            .or_insert(at);
    };

    for dns in &records.dns_observations {
        let src = host_entity(&dns.target);
        for ip in dns.a.iter().chain(dns.aaaa.iter()) {
            bump_latest(&src.id, RelationshipKind::ResolvesTo, dns.checked_at);
            record_edge(
                &mut acc,
                src.clone(),
                RelationshipKind::ResolvesTo,
                EntityRef::new(EntityKind::IpAddress, ip.trim()),
                SignalCategory::DirectlyObserved,
                &dns.id,
                &dns.source,
                dns.checked_at,
                None,
            );
        }
        for cname in &dns.cname {
            let target = host_entity(cname);
            if !target.id.is_empty() {
                bump_latest(&src.id, RelationshipKind::AliasesTo, dns.checked_at);
                record_edge(
                    &mut acc,
                    src.clone(),
                    RelationshipKind::AliasesTo,
                    target,
                    SignalCategory::DirectlyObserved,
                    &dns.id,
                    &dns.source,
                    dns.checked_at,
                    None,
                );
            }
        }
        for ns in &dns.ns {
            let target = host_entity(ns);
            if !target.id.is_empty() {
                bump_latest(&src.id, RelationshipKind::UsesNameserver, dns.checked_at);
                record_edge(
                    &mut acc,
                    src.clone(),
                    RelationshipKind::UsesNameserver,
                    target,
                    SignalCategory::DirectlyObserved,
                    &dns.id,
                    &dns.source,
                    dns.checked_at,
                    Some(
                        "Shared nameservers may reflect DNS hosting rather than common ownership"
                            .to_string(),
                    ),
                );
            }
        }
        for mx in &dns.mx {
            // MX entries can be "10 mail.example.com" or "mail.example.com".
            let host_part = mx.split_whitespace().last().unwrap_or(mx);
            let target = host_entity(host_part);
            if !target.id.is_empty() {
                bump_latest(&src.id, RelationshipKind::RoutesMailTo, dns.checked_at);
                record_edge(
                    &mut acc,
                    src.clone(),
                    RelationshipKind::RoutesMailTo,
                    target,
                    SignalCategory::DirectlyObserved,
                    &dns.id,
                    &dns.source,
                    dns.checked_at,
                    Some(
                        "Shared mail exchangers may reflect a third-party email provider"
                            .to_string(),
                    ),
                );
            }
        }
    }

    for http in &records.http_observations {
        let src = host_entity(&http.target);
        if let Some(tls) = &http.tls {
            if !tls.fingerprint_sha256.is_empty() {
                bump_latest(
                    &src.id,
                    RelationshipKind::PresentsCertificate,
                    http.checked_at,
                );
                let cert =
                    EntityRef::new(EntityKind::TlsCertificate, tls.fingerprint_sha256.clone())
                        .with_label(tls.subject_dn.clone());
                record_edge(
                    &mut acc,
                    src.clone(),
                    RelationshipKind::PresentsCertificate,
                    cert,
                    SignalCategory::DirectlyObserved,
                    &http.id,
                    &http.source,
                    http.checked_at,
                    Some("Shared certificates on CDNs or multi-tenant platforms do not prove common ownership".to_string()),
                );
            }
        }
        for hop in &http.redirect_chain {
            if let Some(target_host) = extract_host(&hop.redirect_to) {
                let target_norm = normalise_host(&target_host);
                if !target_norm.is_empty() && target_norm != src.id {
                    bump_latest(&src.id, RelationshipKind::RedirectsTo, http.checked_at);
                    record_edge(
                        &mut acc,
                        src.clone(),
                        RelationshipKind::RedirectsTo,
                        host_entity(&target_norm),
                        SignalCategory::DirectlyObserved,
                        &http.id,
                        &http.source,
                        http.checked_at,
                        None,
                    );
                }
            }
        }
        if let Some(final_url) = &http.final_url {
            if let Some(target_host) = extract_host(final_url) {
                let target_norm = normalise_host(&target_host);
                if !target_norm.is_empty() && target_norm != src.id {
                    bump_latest(&src.id, RelationshipKind::RedirectsTo, http.checked_at);
                    record_edge(
                        &mut acc,
                        src.clone(),
                        RelationshipKind::RedirectsTo,
                        host_entity(&target_norm),
                        SignalCategory::DirectlyObserved,
                        &http.id,
                        &http.source,
                        http.checked_at,
                        None,
                    );
                }
            }
        }
    }

    for asn in &records.asn_observations {
        let ip_ref = EntityRef::new(EntityKind::IpAddress, asn.target.trim());
        let asn_id = format!("AS{}", asn.asn);
        let asn_ref = EntityRef::new(EntityKind::Asn, asn_id).with_label(asn.org.clone());
        bump_latest(&ip_ref.id, RelationshipKind::BelongsToAsn, asn.checked_at);
        record_edge(
            &mut acc,
            ip_ref,
            RelationshipKind::BelongsToAsn,
            asn_ref,
            SignalCategory::DirectlyObserved,
            &asn.id,
            &asn.source,
            asn.checked_at,
            Some("Shared ASN indicates network routing, not common ownership".to_string()),
        );
    }

    for hosting in &records.hosting_observations {
        if let Some(provider) = &hosting.provider_name {
            let src = host_entity(&hosting.target);
            let target = EntityRef::new(EntityKind::Provider, provider.to_lowercase())
                .with_label(provider.clone());
            bump_latest(&src.id, RelationshipKind::HostedBy, hosting.checked_at);
            let uncertainty = if hosting.cdn_detected {
                Some(
                    "CDN/shared-hosting provider; never implies common ownership across tenants"
                        .to_string(),
                )
            } else {
                Some("Hosting attribution is an assessment from network/header clues".to_string())
            };
            record_edge(
                &mut acc,
                src,
                RelationshipKind::HostedBy,
                target,
                SignalCategory::WeakSharedClue,
                &hosting.id,
                &hosting.source,
                hosting.checked_at,
                uncertainty,
            );
        }
    }

    acc.into_values()
        .map(|e| {
            let is_current = latest_at
                .get(&(e.source.id.clone(), e.kind))
                .is_some_and(|latest| e.last_seen >= *latest);
            ObservedRelationship {
                source: e.source,
                kind: e.kind,
                target: e.target,
                signal: e.signal,
                evidence: RelationshipEvidence {
                    observation_ids: e.observation_ids.into_iter().collect(),
                    sources: e.sources,
                    first_seen: e.first_seen,
                    last_seen: e.last_seen,
                    uncertainty: e.uncertainty,
                },
                is_current,
            }
        })
        .collect()
}

fn extract_host(url_or_host: &str) -> Option<String> {
    let trimmed = url_or_host.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let authority = without_scheme.split('/').next()?;
    let host = authority.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// Shared-infrastructure clue between the queried entity and another entity
/// via a common intermediate (IP, certificate, ASN, CNAME, NS, MX).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedInfrastructureClue {
    pub other_entity: EntityRef,
    pub via: Vec<(RelationshipKind, EntityRef)>,
    pub signal: SignalCategory,
    pub uncertainty: String,
}

/// Result of `balebos related <target>`.
#[derive(Debug, Clone)]
pub struct RelatedReport {
    pub query: String,
    pub direct: Vec<ObservedRelationship>,
    pub shared_clues: Vec<SharedInfrastructureClue>,
}

/// Find all direct relationships and shared-infrastructure clues for `query`.
pub fn related_entities(records: &Records, query: &str) -> RelatedReport {
    let norm = normalise_query(query);
    let edges = project_relationships(records);

    let direct: Vec<ObservedRelationship> = edges
        .iter()
        .filter(|e| e.source.id == norm || e.target.id == norm)
        .cloned()
        .collect();

    // Find other entities that point to the same intermediate targets
    // (IP, TLS certificate, CNAME, NS, MX, redirect target).
    let mut by_shared_target: BTreeMap<String, Vec<(RelationshipKind, EntityRef, EntityRef)>> =
        BTreeMap::new();
    let query_targets: BTreeSet<(RelationshipKind, String)> = edges
        .iter()
        .filter(|e| e.source.id == norm && e.kind != RelationshipKind::HostedBy)
        .map(|e| (e.kind, e.target.id.clone()))
        .collect();

    for e in &edges {
        if e.source.id != norm && query_targets.contains(&(e.kind, e.target.id.clone())) {
            by_shared_target
                .entry(e.source.id.clone())
                .or_default()
                .push((e.kind, e.target.clone(), e.source.clone()));
        }
    }

    let shared_clues = by_shared_target
        .into_values()
        .map(|entries| {
            let other_entity = entries[0].2.clone();
            let via: Vec<(RelationshipKind, EntityRef)> = entries
                .into_iter()
                .map(|(k, t, _)| (k, t))
                .collect();
            let distinct_kinds: BTreeSet<RelationshipKind> = via.iter().map(|(k, _)| *k).collect();
            let signal = if distinct_kinds.len() >= 2 {
                SignalCategory::StrongCandidate
            } else {
                SignalCategory::WeakSharedClue
            };
            let uncertainty = if distinct_kinds.len() >= 2 {
                "Multiple independent technical signals match; still requires operator verification for ownership".to_string()
            } else {
                "Single shared infrastructure signal; may be CDN, shared hosting or third-party service".to_string()
            };
            SharedInfrastructureClue {
                other_entity,
                via,
                signal,
                uncertainty,
            }
        })
        .collect();

    RelatedReport {
        query: norm,
        direct,
        shared_clues,
    }
}

/// Explanation of how two entities are connected (directly or via a shared intermediate).
#[derive(Debug, Clone)]
pub struct ExplainReport {
    pub entity_a: String,
    pub entity_b: String,
    pub direct_edges: Vec<ObservedRelationship>,
    pub shared_paths: Vec<(ObservedRelationship, ObservedRelationship)>,
    pub summary: String,
}

/// Explain any observed or shared-infrastructure connection between `a` and `b`.
pub fn explain_connection(records: &Records, a: &str, b: &str) -> ExplainReport {
    let norm_a = normalise_query(a);
    let norm_b = normalise_query(b);
    let edges = project_relationships(records);

    let direct_edges: Vec<ObservedRelationship> = edges
        .iter()
        .filter(|e| {
            (e.source.id == norm_a && e.target.id == norm_b)
                || (e.source.id == norm_b && e.target.id == norm_a)
        })
        .cloned()
        .collect();

    let mut shared_paths = Vec::new();
    let edges_a: Vec<&ObservedRelationship> =
        edges.iter().filter(|e| e.source.id == norm_a).collect();
    let edges_b: Vec<&ObservedRelationship> =
        edges.iter().filter(|e| e.source.id == norm_b).collect();

    for ea in &edges_a {
        for eb in &edges_b {
            if ea.target.id == eb.target.id && ea.kind == eb.kind {
                shared_paths.push(((*ea).clone(), (*eb).clone()));
            }
        }
    }

    let summary = if !direct_edges.is_empty() {
        format!(
            "Directly observed {} relationship(s) between {} and {}.",
            direct_edges.len(),
            norm_a,
            norm_b
        )
    } else if !shared_paths.is_empty() {
        format!(
            "Found {} shared-infrastructure path(s) between {} and {}; shared infrastructure alone does not prove common ownership.",
            shared_paths.len(),
            norm_a,
            norm_b
        )
    } else {
        format!(
            "No direct or single-hop shared relationship observed between {} and {}.",
            norm_a, norm_b
        )
    };

    ExplainReport {
        entity_a: norm_a,
        entity_b: norm_b,
        direct_edges,
        shared_paths,
        summary,
    }
}

/// Chronological history of all current and past relationships for `query`.
pub fn entity_history(records: &Records, query: &str) -> Vec<ObservedRelationship> {
    let norm = normalise_query(query);
    let mut edges: Vec<ObservedRelationship> = project_relationships(records)
        .into_iter()
        .filter(|e| e.source.id == norm || e.target.id == norm)
        .collect();
    edges.sort_by(|a, b| {
        a.evidence
            .first_seen
            .cmp(&b.evidence.first_seen)
            .then_with(|| a.evidence.last_seen.cmp(&b.evidence.last_seen))
    });
    edges
}

fn normalise_query(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.to_ascii_uppercase().starts_with("AS")
        && trimmed[2..].chars().all(|c| c.is_ascii_digit())
    {
        trimmed.to_ascii_uppercase()
    } else if trimmed.parse::<std::net::IpAddr>().is_ok() {
        trimmed.to_string()
    } else {
        normalise_host(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::probe::types::{
        AsnObservation, DnsObservation, HttpObservation, TlsObservation,
    };
    use crate::inventory::ItemType;

    fn dns_obs(id: &str, target: &str, ips: &[&str], at: DateTime<Utc>) -> DnsObservation {
        DnsObservation {
            item_type: ItemType::DnsRecord,
            id: id.to_string(),
            asset_type: "domain".to_string(),
            target: target.to_string(),
            a: ips.iter().map(|s| s.to_string()).collect(),
            aaaa: vec![],
            cname: vec![],
            mx: vec![],
            ns: vec![],
            txt: vec![],
            soa: vec![],
            checked_at: at,
            source: "dnsx".to_string(),
        }
    }

    #[test]
    fn projects_many_to_many_shared_ip_and_history() {
        let t1 = Utc::now() - chrono::Duration::hours(48);
        let t2 = Utc::now() - chrono::Duration::hours(1);

        let mut records = Records::default();
        records
            .dns_observations
            .push(dns_obs("obs-1", "alpha.example.com", &["203.0.113.10"], t1));
        records
            .dns_observations
            .push(dns_obs("obs-2", "alpha.example.com", &["203.0.113.20"], t2));
        records
            .dns_observations
            .push(dns_obs("obs-3", "beta.example.net", &["203.0.113.10"], t1));

        let rel_ip = related_entities(&records, "203.0.113.10");
        assert_eq!(rel_ip.direct.len(), 2);

        let alpha_hist = entity_history(&records, "alpha.example.com");
        assert_eq!(alpha_hist.len(), 2);
        assert!(!alpha_hist[0].is_current);
        assert!(alpha_hist[1].is_current);

        let explained = explain_connection(&records, "alpha.example.com", "beta.example.net");
        assert_eq!(explained.shared_paths.len(), 1);
        assert!(explained
            .summary
            .contains("does not prove common ownership"));
    }

    #[test]
    fn shared_ip_plus_shared_cert_upgrades_to_strong_candidate() {
        let now = Utc::now();
        let mut records = Records::default();
        records
            .dns_observations
            .push(dns_obs("d1", "one.example.com", &["203.0.113.10"], now));
        records
            .dns_observations
            .push(dns_obs("d2", "two.example.net", &["203.0.113.10"], now));

        for (id, host) in [("h1", "one.example.com"), ("h2", "two.example.net")] {
            records.http_observations.push(HttpObservation {
                item_type: ItemType::HttpCheck,
                id: id.to_string(),
                asset_type: "domain".to_string(),
                target: host.to_string(),
                status: Some(200),
                response_time_ms: 10,
                final_url: None,
                redirect_chain: vec![],
                tls: Some(TlsObservation {
                    subject_dn: "CN=shared".to_string(),
                    issuer_dn: "CN=CA".to_string(),
                    not_before: now,
                    not_after: now + chrono::Duration::days(90),
                    fingerprint_sha256: "deadbeef".to_string(),
                }),
                tech: vec![],
                webserver: None,
                title: None,
                cdn_name: None,
                favicon_hash: None,
                checked_at: now,
                source: "httpx".to_string(),
                error_kind: None,
                error_message: None,
            });
        }

        records.asn_observations.push(AsnObservation {
            item_type: ItemType::AsnRecord,
            id: "a1".to_string(),
            target: "203.0.113.10".to_string(),
            host: "one.example.com".to_string(),
            asn: 64500,
            org: "ExampleNet".to_string(),
            country: "GB".to_string(),
            checked_at: now,
            source: "asnmap".to_string(),
        });

        let report = related_entities(&records, "one.example.com");
        assert_eq!(report.shared_clues.len(), 1);
        assert_eq!(
            report.shared_clues[0].signal,
            SignalCategory::StrongCandidate
        );
    }
}
