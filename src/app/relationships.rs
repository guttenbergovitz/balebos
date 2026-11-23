//! Deterministic relationship projections and investigation queries.
//!
//! Projects typed, evidence-backed relationships directly from immutable
//! `Records` without mutating historical data or conflating shared
//! infrastructure with common ownership (ADR 0011).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, Utc};

use crate::domain::normalize::normalize_fqdn;
use crate::domain::relationship::{
    EntityKind, EntityRef, ObservedRelationship, RelationshipEvidence, RelationshipKind,
    SignalCategory,
};
use crate::domain::service::{ServiceObservationState, VerificationStatus};
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
    historical_only: bool,
}

fn normalise_host(raw: &str) -> String {
    normalize_fqdn(raw)
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
                historical_only: false,
            }
        });
}

/// Project all persisted observations into deterministic, evidence-backed
/// relationships (including historical vs current status).
pub fn project_relationships(records: &Records) -> Vec<ObservedRelationship> {
    project_relationships_before(records, None)
}

/// Project persisted observations at or before `before` into deterministic,
/// evidence-backed relationships.
pub fn project_relationships_before(
    records: &Records,
    before: Option<DateTime<Utc>>,
) -> Vec<ObservedRelationship> {
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
        if before.is_some_and(|cutoff| dns.checked_at > cutoff) {
            continue;
        }
        let src = host_entity(&dns.target);
        // Always advance latest_at for DNS relationship kinds on a valid DNS
        // observation so an empty re-observation marks prior edges historical.
        bump_latest(&src.id, RelationshipKind::ResolvesTo, dns.checked_at);
        bump_latest(&src.id, RelationshipKind::AliasesTo, dns.checked_at);
        bump_latest(&src.id, RelationshipKind::UsesNameserver, dns.checked_at);
        bump_latest(&src.id, RelationshipKind::RoutesMailTo, dns.checked_at);

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
        if before.is_some_and(|cutoff| http.checked_at > cutoff) {
            continue;
        }
        let src = host_entity(&http.target);
        if http.status.is_some() {
            // A successful HTTP observation without TLS/redirects supersedes
            // prior edges; failed probes (status == None) do not erase history.
            bump_latest(
                &src.id,
                RelationshipKind::PresentsCertificate,
                http.checked_at,
            );
            bump_latest(&src.id, RelationshipKind::RedirectsTo, http.checked_at);
        }
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
        if before.is_some_and(|cutoff| asn.checked_at > cutoff) {
            continue;
        }
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
        if before.is_some_and(|cutoff| hosting.checked_at > cutoff) {
            continue;
        }
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

    for ec in &records.enrichment_candidates {
        let ts = ec
            .candidate
            .source_recorded_at
            .unwrap_or(ec.candidate.ingested_at);
        if before.is_some_and(|cutoff| ts > cutoff) {
            continue;
        }
        if let Some(ip) = &ec.candidate.related_ip {
            let src = host_entity(&ec.candidate.entity.id);
            if !src.id.is_empty() {
                bump_latest(&src.id, RelationshipKind::ResolvesTo, ts);
                record_edge(
                    &mut acc,
                    src,
                    RelationshipKind::ResolvesTo,
                    EntityRef::new(EntityKind::IpAddress, ip.trim()),
                    SignalCategory::WeakSharedClue,
                    &ec.id,
                    &ec.candidate.source_id,
                    ts,
                    Some(ec.candidate.note.clone()),
                );
            }
        }
    }

    for svc in &records.service_observations {
        let ts = svc.source_recorded_at.unwrap_or(svc.ingested_at);
        if before.is_some_and(|cutoff| ts > cutoff) {
            continue;
        }
        let svc_ref = EntityRef::new(EntityKind::NetworkService, svc.endpoint.canonical_id());
        let is_hist = svc.state == ServiceObservationState::HistoricalOnly;
        let edge_signal = match svc.verification {
            VerificationStatus::Observed | VerificationStatus::Verified => {
                SignalCategory::DirectlyObserved
            }
            VerificationStatus::OperatorConfirmed => SignalCategory::OperatorConfirmed,
            _ if is_hist => SignalCategory::InsufficientEvidence,
            _ => SignalCategory::WeakSharedClue,
        };

        if let Some(ip) = &svc.host_ip {
            let ip_ref = EntityRef::new(EntityKind::IpAddress, ip.trim());
            if !is_hist {
                bump_latest(&ip_ref.id, RelationshipKind::ExposesService, ts);
            }
            record_edge(
                &mut acc,
                ip_ref.clone(),
                RelationshipKind::ExposesService,
                svc_ref.clone(),
                edge_signal,
                &svc.id,
                &svc.source,
                ts,
                svc.uncertainty.clone(),
            );
            if is_hist {
                if let Some(entry) = acc.get_mut(&EdgeKey {
                    source_kind: EntityKind::IpAddress,
                    source_id: ip_ref.id,
                    kind: RelationshipKind::ExposesService,
                    target_kind: EntityKind::NetworkService,
                    target_id: svc_ref.id.clone(),
                }) {
                    entry.historical_only = true;
                }
            }
        }

        for host in &svc.associated_hosts {
            let h_ref = host_entity(host);
            if !h_ref.id.is_empty() {
                if !is_hist {
                    bump_latest(&h_ref.id, RelationshipKind::ExposesService, ts);
                }
                record_edge(
                    &mut acc,
                    h_ref,
                    RelationshipKind::ExposesService,
                    svc_ref.clone(),
                    SignalCategory::WeakSharedClue,
                    &svc.id,
                    &svc.source,
                    ts,
                    svc.uncertainty.clone(),
                );
            }
        }

        if svc.protocol_id.is_corroborated() {
            if let Some(proto) = svc.protocol_id.protocol() {
                let proto_ref = EntityRef::new(
                    EntityKind::ApplicationInterface,
                    proto.to_string().to_ascii_lowercase(),
                )
                .with_label(proto.to_string());
                if !is_hist {
                    bump_latest(&svc_ref.id, RelationshipKind::SpeaksProtocol, ts);
                }
                record_edge(
                    &mut acc,
                    svc_ref.clone(),
                    RelationshipKind::SpeaksProtocol,
                    proto_ref,
                    edge_signal,
                    &svc.id,
                    &svc.source,
                    ts,
                    svc.uncertainty.clone(),
                );
            }
        }

        for fp in &svc.fingerprints {
            match fp.kind {
                crate::domain::service::FingerprintKind::TlsCertSha256 => {
                    let cert_ref =
                        EntityRef::new(EntityKind::TlsCertificate, fp.value.to_ascii_lowercase());
                    if !is_hist {
                        bump_latest(&svc_ref.id, RelationshipKind::PresentsCertificate, ts);
                    }
                    record_edge(
                        &mut acc,
                        svc_ref.clone(),
                        RelationshipKind::PresentsCertificate,
                        cert_ref,
                        edge_signal,
                        &svc.id,
                        &svc.source,
                        ts,
                        Some("TLS certificate on network service".to_string()),
                    );
                }
                crate::domain::service::FingerprintKind::SshHostKey => {
                    let ssh_ref = EntityRef::new(EntityKind::SshHostKey, fp.value.clone());
                    if !is_hist {
                        bump_latest(&svc_ref.id, RelationshipKind::SharesSshHostKey, ts);
                    }
                    record_edge(
                        &mut acc,
                        svc_ref.clone(),
                        RelationshipKind::SharesSshHostKey,
                        ssh_ref,
                        SignalCategory::StrongCandidate,
                        &svc.id,
                        &svc.source,
                        ts,
                        Some(
                            "SSH host public key fingerprint (strong infrastructure clue, not automatic ownership)"
                                .to_string(),
                        ),
                    );
                }
                _ => {}
            }
        }
    }

    for aref in &records.application_references {
        let ts = aref.source_recorded_at.unwrap_or(aref.ingested_at);
        if before.is_some_and(|cutoff| ts > cutoff) {
            continue;
        }
        let src = host_entity(&aref.origin_asset);
        if src.id.is_empty() {
            continue;
        }
        let target = EntityRef::new(aref.target_kind, aref.referenced_target.clone());
        let signal = if aref.deprecated_hint || aref.dev_or_placeholder_hint {
            SignalCategory::InsufficientEvidence
        } else {
            SignalCategory::WeakSharedClue
        };
        bump_latest(&src.id, aref.relationship_kind, ts);
        record_edge(
            &mut acc,
            src,
            aref.relationship_kind,
            target,
            signal,
            &aref.id,
            &aref.source,
            ts,
            aref.uncertainty.clone(),
        );
    }

    acc.into_values()
        .map(|e| {
            let is_current = !e.historical_only
                && latest_at
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

/// Independent signal planes used to avoid treating correlated attributes
/// (such as IP + TLS cert on one CDN) as independent confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SignalPlane {
    NetworkAddress,
    TlsIdentity,
    DnsDelegation,
    HttpNavigation,
    ServiceExposure,
    SshIdentity,
    ApplicationReference,
}

fn signal_plane(kind: RelationshipKind) -> SignalPlane {
    match kind {
        RelationshipKind::ResolvesTo | RelationshipKind::BelongsToAsn => {
            SignalPlane::NetworkAddress
        }
        RelationshipKind::PresentsCertificate => SignalPlane::TlsIdentity,
        RelationshipKind::AliasesTo
        | RelationshipKind::UsesNameserver
        | RelationshipKind::RoutesMailTo => SignalPlane::DnsDelegation,
        RelationshipKind::RedirectsTo | RelationshipKind::HostedBy => SignalPlane::HttpNavigation,
        RelationshipKind::ExposesService | RelationshipKind::SpeaksProtocol => {
            SignalPlane::ServiceExposure
        }
        RelationshipKind::SharesSshHostKey => SignalPlane::SshIdentity,
        RelationshipKind::ReferencesApi
        | RelationshipKind::LoadsResourceFrom
        | RelationshipKind::ReferencesStorage
        | RelationshipKind::UsesIdentityProvider
        | RelationshipKind::DeclaresService => SignalPlane::ApplicationReference,
    }
}

fn edges_overlap(a: &ObservedRelationship, b: &ObservedRelationship) -> bool {
    a.evidence.first_seen <= b.evidence.last_seen && b.evidence.first_seen <= a.evidence.last_seen
}

fn is_shared_cdn_or_platform(records: &Records, host_a: &str, host_b: &str) -> bool {
    let hosts = [host_a, host_b];
    records
        .hosting_observations
        .iter()
        .any(|h| hosts.contains(&normalise_host(&h.target).as_str()) && h.cdn_detected)
        || records
            .http_observations
            .iter()
            .any(|h| hosts.contains(&normalise_host(&h.target).as_str()) && h.cdn_name.is_some())
}

/// Shared-infrastructure clue between the queried entity and another entity
/// via a common intermediate (IP, certificate, ASN, CNAME, NS, MX).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedInfrastructureClue {
    pub other_entity: EntityRef,
    pub via: Vec<(RelationshipKind, EntityRef)>,
    pub signal: SignalCategory,
    pub concurrent: bool,
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

    // Index all non-provider edges of the queried entity by (kind, target_id).
    let query_edges: Vec<&ObservedRelationship> = edges
        .iter()
        .filter(|e| e.source.id == norm && e.kind != RelationshipKind::HostedBy)
        .collect();

    let mut by_shared_target: BTreeMap<
        String,
        Vec<(&ObservedRelationship, &ObservedRelationship)>,
    > = BTreeMap::new();

    for qe in &query_edges {
        for other in &edges {
            if other.source.id != norm && other.kind == qe.kind && other.target.id == qe.target.id {
                by_shared_target
                    .entry(other.source.id.clone())
                    .or_default()
                    .push((qe, other));
            }
        }
    }

    let shared_clues = by_shared_target
        .into_values()
        .map(|pairs| {
            let other_entity = pairs[0].1.source.clone();
            let mut concurrent = false;
            let mut concurrent_planes: BTreeSet<SignalPlane> = BTreeSet::new();
            let mut via = Vec::new();

            for (qe, other) in &pairs {
                via.push((other.kind, other.target.clone()));
                if edges_overlap(qe, other) {
                    concurrent = true;
                    concurrent_planes.insert(signal_plane(other.kind));
                }
            }

            let cdn_only_ip_cert = concurrent_planes.len() == 2
                && concurrent_planes.contains(&SignalPlane::NetworkAddress)
                && concurrent_planes.contains(&SignalPlane::TlsIdentity)
                && is_shared_cdn_or_platform(records, &norm, &other_entity.id);

            let (signal, uncertainty) = if !concurrent {
                (
                    SignalCategory::WeakSharedClue,
                    "Historical-only shared infrastructure (observation windows do not overlap)".to_string(),
                )
            } else if cdn_only_ip_cert {
                (
                    SignalCategory::WeakSharedClue,
                    "Shared IP and TLS certificate behind CDN/shared platform; does not indicate common ownership".to_string(),
                )
            } else if concurrent_planes.len() >= 2 {
                (
                    SignalCategory::StrongCandidate,
                    "Concurrent match across multiple signal planes; still requires operator verification for ownership".to_string(),
                )
            } else {
                (
                    SignalCategory::WeakSharedClue,
                    "Single concurrent shared infrastructure signal; may be CDN, shared hosting or third-party service".to_string(),
                )
            };

            SharedInfrastructureClue {
                other_entity,
                via,
                signal,
                concurrent,
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

/// A shared-infrastructure path between two entities with temporal overlap status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedPath {
    pub left: ObservedRelationship,
    pub right: ObservedRelationship,
    pub concurrent: bool,
}

/// Explanation of how two entities are connected (directly or via a shared intermediate).
#[derive(Debug, Clone)]
pub struct ExplainReport {
    pub entity_a: String,
    pub entity_b: String,
    pub direct_edges: Vec<ObservedRelationship>,
    pub shared_paths: Vec<SharedPath>,
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
                shared_paths.push(SharedPath {
                    left: (*ea).clone(),
                    right: (*eb).clone(),
                    concurrent: edges_overlap(ea, eb),
                });
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
        let concurrent_count = shared_paths.iter().filter(|p| p.concurrent).count();
        format!(
            "Found {} shared-infrastructure path(s) ({} concurrent, {} historical-only) between {} and {}; shared infrastructure alone does not prove common ownership.",
            shared_paths.len(),
            concurrent_count,
            shared_paths.len() - concurrent_count,
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
    } else if let Some((host_port, proto)) = trimmed.rsplit_once('/') {
        let proto_lower = proto.to_ascii_lowercase();
        if (proto_lower == "tcp" || proto_lower == "udp") && host_port.contains(':') {
            format!("{}/{}", host_port.to_ascii_lowercase(), proto_lower)
        } else {
            normalise_host(trimmed)
        }
    } else {
        normalise_host(trimmed)
    }
}

/// Kind of temporal change between baseline and current relationships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipChangeKind {
    Added,
    VerifiedRemoval,
    NotObserved,
}

/// A temporal change in an infrastructure relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipChange {
    pub change_kind: RelationshipChangeKind,
    pub relationship: ObservedRelationship,
    pub detail: String,
}

/// Compare baseline and current relationships, respecting scan coverage so
/// incomplete scans classify missing edges as `NotObserved` rather than
/// `VerifiedRemoval`.
pub fn compare_relationships(
    baseline: &[ObservedRelationship],
    current: &[ObservedRelationship],
    current_complete: bool,
) -> Vec<RelationshipChange> {
    let key_of = |r: &ObservedRelationship| {
        (
            r.source.kind,
            r.source.id.clone(),
            r.kind,
            r.target.kind,
            r.target.id.clone(),
        )
    };

    let baseline_keys: BTreeSet<_> = baseline
        .iter()
        .filter(|r| r.is_current)
        .map(key_of)
        .collect();
    let current_active_keys: BTreeSet<_> = current
        .iter()
        .filter(|r| r.is_current)
        .map(key_of)
        .collect();
    let current_src_kinds: BTreeSet<(String, RelationshipKind)> = current
        .iter()
        .filter(|r| r.is_current)
        .map(|r| (r.source.id.clone(), r.kind))
        .collect();

    let mut changes = Vec::new();

    for rel in current.iter().filter(|r| r.is_current) {
        if !baseline_keys.contains(&key_of(rel)) {
            changes.push(RelationshipChange {
                change_kind: RelationshipChangeKind::Added,
                relationship: rel.clone(),
                detail: format!(
                    "New relationship {} --[{}]--> {}",
                    rel.source, rel.kind, rel.target
                ),
            });
        }
    }

    for rel in baseline.iter().filter(|r| r.is_current) {
        if !current_active_keys.contains(&key_of(rel)) {
            let reobserved_kind = current_src_kinds.contains(&(rel.source.id.clone(), rel.kind));
            if current_complete && reobserved_kind {
                changes.push(RelationshipChange {
                    change_kind: RelationshipChangeKind::VerifiedRemoval,
                    relationship: rel.clone(),
                    detail: format!(
                        "Relationship {} --[{}]--> {} no longer present in newer observation",
                        rel.source, rel.kind, rel.target
                    ),
                });
            } else {
                changes.push(RelationshipChange {
                    change_kind: RelationshipChangeKind::NotObserved,
                    relationship: rel.clone(),
                    detail: format!(
                        "Relationship {} --[{}]--> {} not observed in current window (coverage incomplete or unverified)",
                        rel.source, rel.kind, rel.target
                    ),
                });
            }
        }
    }

    changes
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
            srv: vec![],
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

    #[test]
    fn temporal_relationship_drift_distinguishes_incomplete_from_verified_removal() {
        let t1 = Utc::now() - chrono::Duration::hours(48);
        let t2 = Utc::now() - chrono::Duration::hours(1);

        let mut records = Records::default();
        records
            .dns_observations
            .push(dns_obs("d1", "app.example.com", &["203.0.113.10"], t1));
        records
            .dns_observations
            .push(dns_obs("d2", "app.example.com", &["203.0.113.20"], t2));

        let cutoff = Utc::now() - chrono::Duration::hours(24);
        let baseline = project_relationships_before(&records, Some(cutoff));
        let current = project_relationships_before(&records, None);

        let incomplete = compare_relationships(&baseline, &current, false);
        assert!(incomplete
            .iter()
            .any(|c| c.change_kind == RelationshipChangeKind::Added));
        assert!(incomplete
            .iter()
            .any(|c| c.change_kind == RelationshipChangeKind::NotObserved));

        let complete = compare_relationships(&baseline, &current, true);
        assert!(complete
            .iter()
            .any(|c| c.change_kind == RelationshipChangeKind::VerifiedRemoval));
    }

    #[test]
    fn empty_reobservation_marks_previous_edge_historical() {
        let t1 = Utc::now() - chrono::Duration::hours(24);
        let t2 = Utc::now() - chrono::Duration::minutes(5);

        let mut records = Records::default();
        records
            .dns_observations
            .push(dns_obs("d1", "stale.example.com", &["203.0.113.10"], t1));
        // Later valid DNS observation returns no A/AAAA records.
        records
            .dns_observations
            .push(dns_obs("d2", "stale.example.com", &[], t2));

        let edges = project_relationships(&records);
        assert_eq!(edges.len(), 1);
        assert!(
            !edges[0].is_current,
            "old IP must become historical when a newer DNS check returns no IPs"
        );
    }

    #[test]
    fn non_overlapping_shared_ip_is_marked_historical_only() {
        let t1 = Utc::now() - chrono::Duration::days(10);
        let t2 = Utc::now() - chrono::Duration::days(1);

        let mut records = Records::default();
        records
            .dns_observations
            .push(dns_obs("d1", "old.example.com", &["203.0.113.10"], t1));
        records
            .dns_observations
            .push(dns_obs("d2", "new.example.net", &["203.0.113.10"], t2));

        let rel = related_entities(&records, "old.example.com");
        assert_eq!(rel.shared_clues.len(), 1);
        assert!(!rel.shared_clues[0].concurrent);

        let explained = explain_connection(&records, "old.example.com", "new.example.net");
        assert_eq!(explained.shared_paths.len(), 1);
        assert!(!explained.shared_paths[0].concurrent);
        assert!(explained
            .summary
            .contains("0 concurrent, 1 historical-only"));
    }

    #[test]
    fn shared_cdn_ip_and_cert_remains_weak_shared_clue() {
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
                    subject_dn: "CN=cdn".to_string(),
                    issuer_dn: "CN=CA".to_string(),
                    not_before: now,
                    not_after: now + chrono::Duration::days(90),
                    fingerprint_sha256: "deadbeef".to_string(),
                }),
                tech: vec![],
                webserver: None,
                title: None,
                cdn_name: Some("Cloudflare".to_string()),
                favicon_hash: None,
                checked_at: now,
                source: "httpx".to_string(),
                error_kind: None,
                error_message: None,
            });
        }

        let report = related_entities(&records, "one.example.com");
        assert_eq!(report.shared_clues.len(), 1);
        assert_eq!(
            report.shared_clues[0].signal,
            SignalCategory::WeakSharedClue
        );
    }

    #[test]
    fn www_and_apex_stay_distinct_in_relationships() {
        let now = Utc::now();
        let mut records = Records::default();
        records
            .dns_observations
            .push(dns_obs("d1", "example.com", &["203.0.113.10"], now));
        records
            .dns_observations
            .push(dns_obs("d2", "www.example.com", &["203.0.113.20"], now));

        let apex = related_entities(&records, "example.com");
        assert_eq!(apex.direct.len(), 1);
        assert_eq!(apex.direct[0].source.id, "example.com");
        assert_eq!(apex.direct[0].target.id, "203.0.113.10");

        let www = related_entities(&records, "www.example.com");
        assert_eq!(www.direct.len(), 1);
        assert_eq!(www.direct[0].source.id, "www.example.com");
        assert_eq!(www.direct[0].target.id, "203.0.113.20");
    }
}
