//! Controlled, bounded pivot discovery over persisted observations.
//!
//! Correlates local observations to surface candidate infrastructure
//! within explicit depth, count and scope bounds. Out-of-scope
//! candidates are recorded passively for operator review and must never
//! be actively probed without explicit authorisation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::app::relationships::project_relationships;
use crate::domain::normalize::normalize_fqdn;
use crate::domain::relationship::{EntityRef, RelationshipKind, SignalCategory};
use crate::storage::reader::Records;

/// Known shared/CDN/third-party suffixes whose shared clues are noisy.
const SHARED_PROVIDER_SUFFIXES: &[&str] = &[
    "cloudflare.com",
    "cloudflare.net",
    "cloudfront.net",
    "fastly.net",
    "akamaized.net",
    "akamai.net",
    "edgekey.net",
    "google.com",
    "googlemail.com",
    "outlook.com",
    "protection.outlook.com",
    "amazonaws.com",
    "azurewebsites.net",
];

/// Bounds and scope controls for controlled pivot discovery.
#[derive(Debug, Clone)]
pub struct PivotConfig {
    /// Approved domain suffixes for active probing. Out-of-scope
    /// candidates are still surfaced passively with `in_scope = false`.
    pub approved_domains: BTreeSet<String>,
    pub max_depth: usize,
    pub max_candidates: usize,
    pub suppress_shared_providers: bool,
}

impl Default for PivotConfig {
    fn default() -> Self {
        Self {
            approved_domains: BTreeSet::new(),
            max_depth: 2,
            max_candidates: 50,
            suppress_shared_providers: true,
        }
    }
}

/// A candidate infrastructure entity discovered by local correlation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PivotCandidate {
    pub candidate: EntityRef,
    pub discovered_from: EntityRef,
    pub via_relationship: RelationshipKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_intermediate: Option<EntityRef>,
    pub depth: usize,
    /// False when outside `approved_domains`: passive review only, never auto-probe.
    pub in_scope: bool,
    pub signal: SignalCategory,
    pub provenance_observation_ids: Vec<String>,
    pub reason: String,
}

/// Result of a bounded pivot discovery pass.
#[derive(Debug, Clone)]
pub struct PivotReport {
    pub seed: String,
    pub candidates: Vec<PivotCandidate>,
    pub suppressed_count: usize,
    pub truncated_by_limit: bool,
}

fn is_shared_provider_host(host: &str) -> bool {
    let norm = normalize_fqdn(host);
    SHARED_PROVIDER_SUFFIXES
        .iter()
        .any(|suffix| norm == *suffix || norm.ends_with(&format!(".{suffix}")))
}

fn is_in_scope(host: &str, approved_domains: &BTreeSet<String>) -> bool {
    if approved_domains.is_empty() {
        return false;
    }
    let norm = normalize_fqdn(host);
    approved_domains
        .iter()
        .any(|root| norm == *root || norm.ends_with(&format!(".{root}")))
}

/// Run bounded local pivot discovery starting from `seed` over `records`.
///
/// Discovers:
/// - `Domain -> CNAME / redirect -> candidate domain` (depth 1)
/// - `Domain -> IP / TLS fingerprint / NS / MX -> other known domains` (depth 2)
///
/// Enforces cycle detection, deduplication, `max_depth`, `max_candidates`,
/// provider-noise suppression, and explicit `in_scope` tagging.
pub fn discover_pivots(records: &Records, seed: &str, config: &PivotConfig) -> PivotReport {
    let seed_norm = normalize_fqdn(seed);
    let edges = project_relationships(records);

    let approved = &config.approved_domains;

    let mut visited: BTreeSet<String> = BTreeSet::new();
    visited.insert(seed_norm.clone());

    let mut frontier: Vec<(String, usize)> = vec![(seed_norm.clone(), 0)];
    let mut candidates: Vec<PivotCandidate> = Vec::new();
    let mut suppressed_count = 0usize;
    let mut truncated_by_limit = false;

    while let Some((current, depth)) = frontier.first().cloned() {
        frontier.remove(0);
        if depth >= config.max_depth {
            continue;
        }

        // 1-hop host pivots (CNAME, Redirect, NS, MX)
        for edge in edges.iter().filter(|e| e.source.id == current) {
            if candidates.len() >= config.max_candidates {
                truncated_by_limit = true;
                break;
            }

            let next_depth = depth + 1;
            match edge.kind {
                RelationshipKind::AliasesTo
                | RelationshipKind::RedirectsTo
                | RelationshipKind::UsesNameserver
                | RelationshipKind::RoutesMailTo => {
                    let target_id = edge.target.id.clone();
                    if config.suppress_shared_providers && is_shared_provider_host(&target_id) {
                        suppressed_count += 1;
                        continue;
                    }
                    if !visited.insert(target_id.clone()) {
                        continue;
                    }
                    let in_scope = is_in_scope(&target_id, approved);
                    let signal = match edge.kind {
                        RelationshipKind::AliasesTo | RelationshipKind::RedirectsTo => {
                            SignalCategory::DirectlyObserved
                        }
                        _ => SignalCategory::WeakSharedClue,
                    };
                    candidates.push(PivotCandidate {
                        candidate: edge.target.clone(),
                        discovered_from: edge.source.clone(),
                        via_relationship: edge.kind,
                        via_intermediate: None,
                        depth: next_depth,
                        in_scope,
                        signal,
                        provenance_observation_ids: edge.evidence.observation_ids.clone(),
                        reason: format!("{} --[{}]--> {}", edge.source.id, edge.kind, target_id),
                    });
                    if next_depth < config.max_depth && in_scope {
                        frontier.push((target_id, next_depth));
                    }
                }
                // 2-hop shared-intermediate pivots (IP, TLS cert)
                RelationshipKind::ResolvesTo | RelationshipKind::PresentsCertificate => {
                    if next_depth >= config.max_depth {
                        // Still allow 2-hop correlation within max_depth == 2 if depth == 0
                        if depth + 2 > config.max_depth {
                            continue;
                        }
                    }
                    for peer in edges.iter().filter(|other| {
                        other.kind == edge.kind
                            && other.target.id == edge.target.id
                            && other.source.id != current
                    }) {
                        if candidates.len() >= config.max_candidates {
                            truncated_by_limit = true;
                            break;
                        }
                        let peer_id = peer.source.id.clone();
                        if !visited.insert(peer_id.clone()) {
                            continue;
                        }
                        let in_scope = is_in_scope(&peer_id, approved);
                        let mut obs_ids = edge.evidence.observation_ids.clone();
                        obs_ids.extend(peer.evidence.observation_ids.clone());
                        candidates.push(PivotCandidate {
                            candidate: peer.source.clone(),
                            discovered_from: edge.source.clone(),
                            via_relationship: edge.kind,
                            via_intermediate: Some(edge.target.clone()),
                            depth: depth + 2,
                            in_scope,
                            signal: SignalCategory::WeakSharedClue,
                            provenance_observation_ids: obs_ids,
                            reason: format!(
                                "Shared {} ({}) with {}",
                                edge.target.kind, edge.target.id, current
                            ),
                        });
                    }
                }
                RelationshipKind::BelongsToAsn | RelationshipKind::HostedBy => {}
            }
        }

        if truncated_by_limit {
            break;
        }
    }

    PivotReport {
        seed: seed_norm,
        candidates,
        suppressed_count,
        truncated_by_limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::probe::types::{DnsObservation, HttpObservation, RedirectHop};
    use crate::inventory::{InventoryItem, ItemType};
    use chrono::Utc;

    fn dns_with(
        id: &str,
        target: &str,
        ips: &[&str],
        cnames: &[&str],
        ns: &[&str],
    ) -> DnsObservation {
        DnsObservation {
            item_type: ItemType::DnsRecord,
            id: id.to_string(),
            asset_type: "domain".to_string(),
            target: target.to_string(),
            a: ips.iter().map(|s| s.to_string()).collect(),
            aaaa: vec![],
            cname: cnames.iter().map(|s| s.to_string()).collect(),
            mx: vec![],
            ns: ns.iter().map(|s| s.to_string()).collect(),
            txt: vec![],
            soa: vec![],
            checked_at: Utc::now(),
            source: "dnsx".to_string(),
        }
    }

    #[test]
    fn pivot_enforces_scope_cycle_detection_and_noise_suppression() {
        let now = Utc::now();
        let mut records = Records::default();
        records.items.push(InventoryItem {
            item_type: ItemType::Domain,
            id: "1".to_string(),
            name: "example.com".to_string(),
            discovered_at: now,
            source: "cli".to_string(),
            original_input: None,
        });

        // CNAME to external out-of-scope target + noisy Cloudflare NS.
        records.dns_observations.push(dns_with(
            "d1",
            "example.com",
            &["203.0.113.10"],
            &["external-app.example.net"],
            &["ada.ns.cloudflare.com"],
        ));
        // Another known asset on the same IP.
        records.dns_observations.push(dns_with(
            "d2",
            "portal.example.com",
            &["203.0.113.10"],
            &[],
            &[],
        ));
        // Redirect cycle back to example.com.
        records.http_observations.push(HttpObservation {
            item_type: ItemType::HttpCheck,
            id: "h1".to_string(),
            asset_type: "domain".to_string(),
            target: "external-app.example.net".to_string(),
            status: Some(302),
            response_time_ms: 5,
            final_url: Some("https://example.com/".to_string()),
            redirect_chain: vec![RedirectHop {
                url: "https://external-app.example.net".to_string(),
                status: 302,
                redirect_to: "https://example.com/".to_string(),
            }],
            tls: None,
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

        let mut approved = BTreeSet::new();
        approved.insert("example.com".to_string());
        let cfg = PivotConfig {
            approved_domains: approved,
            max_depth: 2,
            max_candidates: 10,
            suppress_shared_providers: true,
        };

        let report = discover_pivots(&records, "example.com", &cfg);
        assert_eq!(report.suppressed_count, 1, "cloudflare NS suppressed");
        assert!(report
            .candidates
            .iter()
            .any(|c| c.candidate.id == "external-app.example.net" && !c.in_scope));
        assert!(report
            .candidates
            .iter()
            .any(|c| c.candidate.id == "portal.example.com" && c.in_scope));
        assert!(
            !report
                .candidates
                .iter()
                .any(|c| c.candidate.id == "example.com"),
            "seed must not reappear via cycle"
        );
    }

    #[test]
    fn empty_scope_marks_all_candidates_out_of_scope() {
        let mut records = Records::default();
        records.items.push(InventoryItem {
            item_type: ItemType::Domain,
            id: "1".to_string(),
            name: "other.example.net".to_string(),
            discovered_at: Utc::now(),
            source: "cli".to_string(),
            original_input: None,
        });
        records.dns_observations.push(dns_with(
            "d1",
            "seed.example.com",
            &["203.0.113.10"],
            &[],
            &[],
        ));
        records.dns_observations.push(dns_with(
            "d2",
            "other.example.net",
            &["203.0.113.10"],
            &[],
            &[],
        ));

        let report = discover_pivots(&records, "seed.example.com", &PivotConfig::default());
        assert_eq!(report.candidates.len(), 1);
        assert!(
            !report.candidates[0].in_scope,
            "inventory membership alone must never grant active-probing scope"
        );
    }

    #[test]
    fn pivot_respects_max_candidates_limit() {
        let mut records = Records::default();
        records.dns_observations.push(dns_with(
            "d1",
            "seed.example.com",
            &[],
            &["a.example.net", "b.example.net", "c.example.net"],
            &[],
        ));

        let cfg = PivotConfig {
            max_candidates: 2,
            ..Default::default()
        };
        let report = discover_pivots(&records, "seed.example.com", &cfg);
        assert_eq!(report.candidates.len(), 2);
        assert!(report.truncated_by_limit);
    }
}
