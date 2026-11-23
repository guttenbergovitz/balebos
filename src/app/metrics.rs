//! Discovery effectiveness measurement framework (ADR 0022).
//!
//! Separates automatically computable metrics (yield, marginal source
//! contribution, evidence traceability, query latency) from operator-
//! reviewed precision metrics without collapsing them into an opaque score.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::app::pivot::PivotCandidate;
use crate::app::relationships::project_relationships;
use crate::app::sources::EnrichmentCandidate;
use crate::domain::normalize::normalize_fqdn;
use crate::storage::reader::Records;

/// Operator review decision for a surfaced candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateReviewDecision {
    Confirmed,
    Rejected,
    Deferred,
}

/// Per-source discovery yield and unique marginal contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceYield {
    pub source: String,
    pub raw_candidates: usize,
    pub unique_new_candidates: usize,
}

/// Traceability and currency of projected relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceQualityMetrics {
    pub total_relationships: usize,
    pub observation_backed_relationships: usize,
    pub traceable_percentage: f64,
    pub current_relationships: usize,
    pub historical_relationships: usize,
}

/// Precision computed strictly over operator-reviewed candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateReviewMetrics {
    pub total_candidates: usize,
    pub reviewed_count: usize,
    pub confirmed_count: usize,
    pub rejected_count: usize,
    pub deferred_count: usize,
    /// `confirmed / (confirmed + rejected)` when at least one terminal review exists.
    pub reviewed_precision: Option<f64>,
}

/// Service and application-reference discovery effectiveness metrics (ADR 0030).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceDiscoveryMetrics {
    pub unique_service_endpoints: usize,
    pub corroborated_protocols: usize,
    pub management_interfaces: usize,
    pub api_endpoints: usize,
    pub storage_references: usize,
    pub traceable_percentage: f64,
    pub independently_verified_count: usize,
    pub historical_only_count: usize,
    pub uncertain_count: usize,
    pub per_category_reviewed_precision: BTreeMap<String, Option<f64>>,
}

/// Consolidated discovery effectiveness report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryMetricsReport {
    pub known_assets: usize,
    pub total_unique_candidates: usize,
    pub per_source_yield: Vec<SourceYield>,
    pub evidence_quality: EvidenceQualityMetrics,
    pub candidate_review: CandidateReviewMetrics,
    pub service_discovery: ServiceDiscoveryMetrics,
    pub projection_latency_ms: u128,
}

/// Compute discovery effectiveness metrics from `records`, optional pivot and
/// enrichment candidates, and operator review decisions.
pub fn compute_discovery_metrics(
    records: &Records,
    pivot_candidates: &[PivotCandidate],
    enrichment_candidates: &[EnrichmentCandidate],
    reviews: &BTreeMap<String, CandidateReviewDecision>,
) -> DiscoveryMetricsReport {
    let start = Instant::now();
    let relationships = project_relationships(records);
    let projection_latency_ms = start.elapsed().as_millis();

    let known: BTreeSet<String> = records
        .items
        .iter()
        .map(|i| normalize_fqdn(&i.name))
        .collect();

    let mut per_source_raw: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_source_novel: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut all_novel: BTreeSet<String> = BTreeSet::new();

    for item in &records.items {
        *per_source_raw.entry(item.source.clone()).or_insert(0) += 1;
    }

    for p in pivot_candidates {
        let src = format!("pivot:{}", p.via_relationship);
        *per_source_raw.entry(src.clone()).or_insert(0) += 1;
        let id = p.candidate.id.trim_start_matches("*.").to_string();
        if !known.contains(&id) {
            all_novel.insert(id.clone());
            per_source_novel.entry(src).or_default().insert(id);
        }
    }

    for e in enrichment_candidates {
        *per_source_raw.entry(e.source_id.clone()).or_insert(0) += 1;
        let id = e.entity.id.trim_start_matches("*.").to_string();
        if !known.contains(&id) {
            all_novel.insert(id.clone());
            per_source_novel
                .entry(e.source_id.clone())
                .or_default()
                .insert(id);
        }
    }

    let mut unique_services = BTreeSet::new();
    let mut corroborated_protos = BTreeSet::new();
    let mut mgmt_ifaces = BTreeSet::new();
    let mut verified_count = 0usize;
    let mut historical_only_count = 0usize;
    let mut uncertain_count = 0usize;
    let mut traceable_count = 0usize;
    let mut category_reviews: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for svc in &records.service_observations {
        *per_source_raw.entry(svc.source.clone()).or_insert(0) += 1;
        let ep_id = svc.endpoint.canonical_id();
        if unique_services.insert(ep_id.clone()) {
            all_novel.insert(ep_id.clone());
            per_source_novel
                .entry(svc.source.clone())
                .or_default()
                .insert(ep_id.clone());
        }
        if svc.protocol_id.is_corroborated() {
            if let Some(p) = svc.protocol_id.protocol() {
                corroborated_protos.insert(format!("{}@{}", p, ep_id));
            }
        }
        if svc.category == crate::domain::service::ServiceCategory::ManagementInterface {
            mgmt_ifaces.insert(ep_id.clone());
        }
        if matches!(
            svc.verification,
            crate::domain::service::VerificationStatus::Verified
                | crate::domain::service::VerificationStatus::Observed
        ) {
            verified_count += 1;
        }
        if svc.state == crate::domain::service::ServiceObservationState::HistoricalOnly {
            historical_only_count += 1;
        }
        if svc.uncertainty.is_some() {
            uncertain_count += 1;
        }
        if !svc.evidence.is_empty() {
            traceable_count += 1;
        }
        if let Some(dec) = reviews.get(&ep_id) {
            let entry = category_reviews
                .entry(svc.category.to_string())
                .or_insert((0, 0));
            match dec {
                CandidateReviewDecision::Confirmed => entry.0 += 1,
                CandidateReviewDecision::Rejected => entry.1 += 1,
                CandidateReviewDecision::Deferred => {}
            }
        }
    }

    let mut api_endpoints = BTreeSet::new();
    let mut storage_refs = BTreeSet::new();
    for aref in &records.application_references {
        *per_source_raw.entry(aref.source.clone()).or_insert(0) += 1;
        if aref.target_kind == crate::domain::relationship::EntityKind::ApiService {
            api_endpoints.insert(aref.referenced_target.clone());
        } else if aref.target_kind == crate::domain::relationship::EntityKind::StorageEndpoint {
            storage_refs.insert(aref.referenced_target.clone());
        }
        all_novel.insert(aref.referenced_target.clone());
        per_source_novel
            .entry(aref.source.clone())
            .or_default()
            .insert(aref.referenced_target.clone());
        if !aref.evidence.is_empty() {
            traceable_count += 1;
        }
        if aref.uncertainty.is_some() {
            uncertain_count += 1;
        }
        if let Some(dec) = reviews.get(&aref.referenced_target) {
            let cat = if aref.target_kind == crate::domain::relationship::EntityKind::ApiService {
                "api_endpoint"
            } else if aref.target_kind == crate::domain::relationship::EntityKind::StorageEndpoint {
                "storage"
            } else {
                "inferred_dependency"
            };
            let entry = category_reviews.entry(cat.to_string()).or_insert((0, 0));
            match dec {
                CandidateReviewDecision::Confirmed => entry.0 += 1,
                CandidateReviewDecision::Rejected => entry.1 += 1,
                CandidateReviewDecision::Deferred => {}
            }
        }
    }

    let total_svc_and_refs =
        records.service_observations.len() + records.application_references.len();
    let svc_traceable_pct = if total_svc_and_refs == 0 {
        100.0
    } else {
        (traceable_count as f64 / total_svc_and_refs as f64) * 100.0
    };
    let per_category_reviewed_precision = category_reviews
        .into_iter()
        .map(|(cat, (conf, rej))| {
            let p = if conf + rej > 0 {
                Some(conf as f64 / (conf + rej) as f64)
            } else {
                None
            };
            (cat, p)
        })
        .collect();

    let per_source_yield = per_source_raw
        .into_iter()
        .map(|(source, raw_candidates)| {
            let unique_new_candidates = per_source_novel.get(&source).map_or(0, |s| s.len());
            SourceYield {
                source,
                raw_candidates,
                unique_new_candidates,
            }
        })
        .collect();

    let total_rels = relationships.len();
    let backed = relationships
        .iter()
        .filter(|r| !r.evidence.observation_ids.is_empty())
        .count();
    let traceable_percentage = if total_rels == 0 {
        100.0
    } else {
        (backed as f64 / total_rels as f64) * 100.0
    };
    let current_relationships = relationships.iter().filter(|r| r.is_current).count();

    let mut confirmed_count = 0usize;
    let mut rejected_count = 0usize;
    let mut deferred_count = 0usize;
    for decision in reviews.values() {
        match decision {
            CandidateReviewDecision::Confirmed => confirmed_count += 1,
            CandidateReviewDecision::Rejected => rejected_count += 1,
            CandidateReviewDecision::Deferred => deferred_count += 1,
        }
    }
    let terminal = confirmed_count + rejected_count;
    let reviewed_precision = if terminal > 0 {
        Some(confirmed_count as f64 / terminal as f64)
    } else {
        None
    };

    DiscoveryMetricsReport {
        known_assets: known.len(),
        total_unique_candidates: all_novel.len(),
        per_source_yield,
        evidence_quality: EvidenceQualityMetrics {
            total_relationships: total_rels,
            observation_backed_relationships: backed,
            traceable_percentage,
            current_relationships,
            historical_relationships: total_rels.saturating_sub(current_relationships),
        },
        candidate_review: CandidateReviewMetrics {
            total_candidates: all_novel.len(),
            reviewed_count: reviews.len(),
            confirmed_count,
            rejected_count,
            deferred_count,
            reviewed_precision,
        },
        service_discovery: ServiceDiscoveryMetrics {
            unique_service_endpoints: unique_services.len(),
            corroborated_protocols: corroborated_protos.len(),
            management_interfaces: mgmt_ifaces.len(),
            api_endpoints: api_endpoints.len(),
            storage_references: storage_refs.len(),
            traceable_percentage: svc_traceable_pct,
            independently_verified_count: verified_count,
            historical_only_count,
            uncertain_count,
            per_category_reviewed_precision,
        },
        projection_latency_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::pivot::{discover_pivots, PivotConfig};
    use crate::app::probe::types::{DnsObservation, HttpObservation, TlsObservation};
    use crate::app::relationships::{
        compare_relationships, explain_connection, project_relationships_before, related_entities,
        RelationshipChangeKind,
    };
    use crate::app::sources::{parse_ct_json, parse_passive_dns_json};
    use crate::domain::coverage::{ScanReport, TargetCoverage, TargetCoverageRecord};
    use crate::domain::relationship::SignalCategory;
    use crate::inventory::{InventoryItem, ItemType};
    use chrono::Utc;

    fn dns_obs(id: &str, target: &str, ips: &[&str], at: chrono::DateTime<Utc>) -> DnsObservation {
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

    fn http_obs(
        id: &str,
        target: &str,
        cert_fp: Option<&str>,
        cdn: Option<&str>,
        at: chrono::DateTime<Utc>,
    ) -> HttpObservation {
        HttpObservation {
            item_type: ItemType::HttpCheck,
            id: id.to_string(),
            asset_type: "domain".to_string(),
            target: target.to_string(),
            status: Some(200),
            response_time_ms: 15,
            final_url: None,
            redirect_chain: vec![],
            tls: cert_fp.map(|fp| TlsObservation {
                subject_dn: "CN=test".to_string(),
                issuer_dn: "CN=CA".to_string(),
                not_before: at,
                not_after: at + chrono::Duration::days(90),
                fingerprint_sha256: fp.to_string(),
            }),
            tech: vec![],
            webserver: None,
            title: None,
            cdn_name: cdn.map(|s| s.to_string()),
            favicon_hash: None,
            checked_at: at,
            source: "httpx".to_string(),
            error_kind: None,
            error_message: None,
        }
    }

    /// Representative benchmark fixture covering scenarios 1–14 from the
    /// discovery specification and measuring yield, traceability and precision.
    #[test]
    fn representative_14_scenario_benchmark_and_metrics() {
        let now = Utc::now();
        let old = now - chrono::Duration::days(30);
        let mid = now - chrono::Duration::days(2);
        let mut records = Records::default();

        for (i, name) in [
            "a.example.com",
            "b.example.com",
            "cdn.example.com",
            "moved.example.com",
        ]
        .iter()
        .enumerate()
        {
            records.items.push(InventoryItem {
                item_type: ItemType::Domain,
                id: format!("item-{i}"),
                name: (*name).to_string(),
                discovered_at: old,
                source: "subfinder".to_string(),
                original_input: None,
            });
        }

        // Scenarios 1 & 12: concurrent shared IP + shared non-CDN cert.
        records
            .dns_observations
            .push(dns_obs("d-a", "a.example.com", &["203.0.113.10"], now));
        records
            .dns_observations
            .push(dns_obs("d-b", "b.example.com", &["203.0.113.10"], now));
        records.http_observations.push(http_obs(
            "h-a",
            "a.example.com",
            Some("cert-shared-1"),
            None,
            now,
        ));
        records.http_observations.push(http_obs(
            "h-b",
            "b.example.com",
            Some("cert-shared-1"),
            None,
            now,
        ));

        // Scenario 2: different-time shared IP.
        records.dns_observations.push(dns_obs(
            "d-old",
            "legacy.example.org",
            &["198.51.100.7"],
            old,
        ));
        records.dns_observations.push(dns_obs(
            "d-new",
            "fresh.example.net",
            &["198.51.100.7"],
            now,
        ));

        // Scenarios 3 & 4: CDN shared IP + cert must stay WeakSharedClue.
        records
            .dns_observations
            .push(dns_obs("d-cdn1", "cdn.example.com", &["104.16.0.1"], now));
        records.dns_observations.push(dns_obs(
            "d-cdn2",
            "unrelated.example.io",
            &["104.16.0.1"],
            now,
        ));
        records.http_observations.push(http_obs(
            "h-cdn1",
            "cdn.example.com",
            Some("cert-cf"),
            Some("Cloudflare"),
            now,
        ));
        records.http_observations.push(http_obs(
            "h-cdn2",
            "unrelated.example.io",
            Some("cert-cf"),
            Some("Cloudflare"),
            now,
        ));

        // Scenarios 5, 6 & 14: cert renewal + IP move + disappearing/reappearing edge.
        records
            .dns_observations
            .push(dns_obs("d-m1", "moved.example.com", &["192.0.2.1"], old));
        records
            .dns_observations
            .push(dns_obs("d-m2", "moved.example.com", &[], mid));
        records
            .dns_observations
            .push(dns_obs("d-m3", "moved.example.com", &["192.0.2.2"], now));
        records.http_observations.push(http_obs(
            "h-m1",
            "moved.example.com",
            Some("cert-old"),
            None,
            old,
        ));
        records.http_observations.push(http_obs(
            "h-m2",
            "moved.example.com",
            Some("cert-new"),
            None,
            now,
        ));

        // Scenario 10: incomplete scan report.
        records.scan_reports.push(ScanReport::start(now).finish(
            vec!["a.example.com".to_string(), "cdn.example.com".to_string()],
            vec![
                TargetCoverageRecord {
                    target: "a.example.com".to_string(),
                    coverage: TargetCoverage::Observed,
                },
                TargetCoverageRecord {
                    target: "cdn.example.com".to_string(),
                    coverage: TargetCoverage::Failed {
                        error: "timeout".to_string(),
                    },
                },
            ],
            now,
        ));

        // Scenarios 7, 8 & 11: CT & Passive DNS introducing novel + out-of-scope candidates.
        let mut scope = BTreeSet::new();
        scope.insert("example.com".to_string());
        let ct = parse_ct_json(
            br#"[{"id":1,"name_value":"*.wild.example.com\nct-only.example.com","entry_timestamp":"2026-02-01T00:00:00Z"}]"#,
            &scope,
            50,
        )
        .unwrap();
        let pdns = parse_passive_dns_json(
            br#"[{"fqdn":"pdns-only.example.net","ip":"203.0.113.10","source_last_seen":"2025-12-01T00:00:00Z"}]"#,
            &scope,
            50,
        )
        .unwrap();

        let mut enrichment = ct;
        enrichment.extend(pdns);

        let pivot = discover_pivots(
            &records,
            "a.example.com",
            &PivotConfig {
                approved_domains: scope,
                ..Default::default()
            },
        );

        let mut reviews = BTreeMap::new();
        reviews.insert(
            "ct-only.example.com".to_string(),
            CandidateReviewDecision::Confirmed,
        );
        reviews.insert(
            "pdns-only.example.net".to_string(),
            CandidateReviewDecision::Deferred,
        );

        let metrics = compute_discovery_metrics(&records, &pivot.candidates, &enrichment, &reviews);

        assert_eq!(metrics.evidence_quality.traceable_percentage, 100.0);
        assert!(metrics.total_unique_candidates >= 3);
        assert_eq!(metrics.candidate_review.reviewed_precision, Some(1.0));

        // Verify semantic assertions on the fixture:
        let rel_a = related_entities(&records, "a.example.com");
        assert!(rel_a
            .shared_clues
            .iter()
            .any(|c| c.other_entity.id == "b.example.com"
                && c.signal == SignalCategory::StrongCandidate));

        let rel_cdn = related_entities(&records, "cdn.example.com");
        assert!(rel_cdn
            .shared_clues
            .iter()
            .all(|c| c.signal == SignalCategory::WeakSharedClue));

        let exp_hist = explain_connection(&records, "legacy.example.org", "fresh.example.net");
        assert_eq!(exp_hist.shared_paths.len(), 1);
        assert!(!exp_hist.shared_paths[0].concurrent);

        let base = project_relationships_before(&records, Some(mid));
        let curr = project_relationships_before(&records, None);
        let changes = compare_relationships(&base, &curr, false);
        assert!(changes
            .iter()
            .any(|c| c.change_kind == RelationshipChangeKind::NotObserved));
    }

    /// Milestone E2E acceptance test:
    /// Empty DB -> 3 known domains -> discover 4th via Passive DNS -> persist
    /// evidence & review -> reopen DB from disk -> verify linkage, explain,
    /// metrics, and subsequent scan change without losing history.
    #[test]
    fn e2e_empty_db_to_pdns_discovery_explain_reopen_and_drift() {
        use crate::app::sources::{
            novel_candidates, CandidateReviewRecord, EnrichmentCandidateRecord,
        };
        use crate::storage::{SqliteStorage, StorageEngine, StorageEngineExt};
        use std::sync::Arc;

        let dir = std::env::temp_dir().join(format!("balebos_e2e_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("balebos.db");
        let db_str = db_path.to_str().unwrap();

        let t0 = Utc::now() - chrono::Duration::hours(2);
        let t1 = Utc::now() - chrono::Duration::minutes(10);

        {
            let storage: Arc<dyn StorageEngine> = Arc::new(SqliteStorage::new(db_str).unwrap());
            for (i, name) in ["one.example.com", "two.example.com", "three.example.com"]
                .iter()
                .enumerate()
            {
                storage
                    .save(&InventoryItem {
                        item_type: ItemType::Domain,
                        id: format!("seed-{i}"),
                        name: (*name).to_string(),
                        discovered_at: t0,
                        source: "cli".to_string(),
                        original_input: None,
                    })
                    .unwrap();
            }
            storage
                .save(&dns_obs(
                    "dns-one",
                    "one.example.com",
                    &["203.0.113.10"],
                    t0,
                ))
                .unwrap();

            let mut scope = BTreeSet::new();
            scope.insert("example.com".to_string());
            let pdns_json = format!(
                r#"[{{"fqdn":"fourth.example.com","ip":"203.0.113.10","source_last_seen":"{}","provider":"fixture-pdns"}}]"#,
                t0.to_rfc3339()
            );
            let candidates = parse_passive_dns_json(pdns_json.as_bytes(), &scope, 50).unwrap();
            let initial = storage.read_all_records().unwrap();
            let novel = novel_candidates(&initial, &candidates);
            assert_eq!(novel.len(), 1);

            storage
                .save(&EnrichmentCandidateRecord {
                    item_type: ItemType::EnrichmentCandidate,
                    id: "ec-1".to_string(),
                    candidate: novel[0].clone(),
                })
                .unwrap();
            storage
                .save(&CandidateReviewRecord {
                    item_type: ItemType::CandidateReview,
                    id: "rev-1".to_string(),
                    entity_id: "fourth.example.com".to_string(),
                    decision: CandidateReviewDecision::Confirmed,
                    reviewed_at: t0,
                    note: None,
                })
                .unwrap();
        }

        // Reopen DB from disk and verify persistence, explanation and metrics.
        {
            let storage: std::sync::Arc<dyn crate::storage::StorageEngine> =
                std::sync::Arc::new(SqliteStorage::new(db_str).unwrap());
            let records = storage.read_all_records().unwrap();
            assert_eq!(records.items.len(), 3);
            assert_eq!(records.enrichment_candidates.len(), 1);
            assert_eq!(records.candidate_reviews.len(), 1);

            let exp = explain_connection(&records, "one.example.com", "fourth.example.com");
            assert_eq!(exp.shared_paths.len(), 1);
            assert!(exp.shared_paths[0].concurrent);

            let enrichment: Vec<_> = records
                .enrichment_candidates
                .iter()
                .map(|r| r.candidate.clone())
                .collect();
            let mut reviews = BTreeMap::new();
            for r in &records.candidate_reviews {
                reviews.insert(r.entity_id.clone(), r.decision);
            }
            let metrics = compute_discovery_metrics(&records, &[], &enrichment, &reviews);
            assert_eq!(metrics.known_assets, 3);
            assert_eq!(metrics.total_unique_candidates, 1);
            assert_eq!(metrics.candidate_review.reviewed_precision, Some(1.0));

            // Subsequent scan moves one.example.com to a new IP without losing history.
            storage
                .save(&dns_obs(
                    "dns-one-v2",
                    "one.example.com",
                    &["203.0.113.99"],
                    t1,
                ))
                .unwrap();
            let updated = storage.read_all_records().unwrap();
            let rels = related_entities(&updated, "one.example.com");
            assert_eq!(rels.direct.len(), 2);
            assert!(rels
                .direct
                .iter()
                .any(|r| !r.is_current && r.target.id == "203.0.113.10"));
            assert!(rels
                .direct
                .iter()
                .any(|r| r.is_current && r.target.id == "203.0.113.99"));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
