//! CLI handlers for evidence-backed relationship queries (`related`,
//! `explain`, `history`).

use std::sync::Arc;

use crate::app::metrics::{compute_discovery_metrics, CandidateReviewDecision};
use crate::app::pivot::{discover_pivots, PivotConfig};
use crate::app::relationships::{
    compare_relationships, entity_history, explain_connection, project_relationships_before,
    related_entities, RelationshipChangeKind,
};
use crate::app::snapshot::latest_scan_is_complete;
use crate::app::sources::{
    novel_candidates, parse_ct_json, parse_passive_dns_json, registered_sources,
    CandidateReviewRecord, EnrichmentCandidateRecord,
};
use crate::domain::normalize::normalize_fqdn;
use crate::error::{BalebosError, Result};
use crate::inventory::ItemType;
use crate::storage::{StorageEngine, StorageEngineExt};

/// Display direct relationships and shared-infrastructure clues for `target`.
pub fn cmd_related(target: &str, storage: &Arc<dyn StorageEngine>) -> Result<()> {
    let records = storage.read_all_records()?;
    let report = related_entities(&records, target);

    println!("Related entities for: {}", report.query);
    if report.direct.is_empty() {
        println!("  No direct relationships observed.");
    } else {
        println!("\nDirect relationships ({}):", report.direct.len());
        for rel in &report.direct {
            let state = if rel.is_current {
                "current"
            } else {
                "historical"
            };
            let sources: Vec<&str> = rel.evidence.sources.iter().map(String::as_str).collect();
            println!(
                "  {} --[{}]--> {}  [{}, {}, {} obs, {}..{}, via {}]",
                rel.source,
                rel.kind,
                rel.target,
                state,
                rel.signal,
                rel.evidence.observation_ids.len(),
                rel.evidence.first_seen.format("%Y-%m-%d %H:%M"),
                rel.evidence.last_seen.format("%Y-%m-%d %H:%M"),
                sources.join(", ")
            );
            if let Some(note) = &rel.evidence.uncertainty {
                println!("    note: {note}");
            }
        }
    }

    if !report.shared_clues.is_empty() {
        println!(
            "\nShared infrastructure clues ({}):",
            report.shared_clues.len()
        );
        for clue in &report.shared_clues {
            let via: Vec<String> = clue.via.iter().map(|(k, t)| format!("{k} {t}")).collect();
            let timing = if clue.concurrent {
                "concurrent"
            } else {
                "historical-only"
            };
            println!(
                "  {}  [{}, {}] via {}",
                clue.other_entity,
                clue.signal,
                timing,
                via.join(" + ")
            );
            println!("    uncertainty: {}", clue.uncertainty);
        }
    }

    Ok(())
}

/// Explain any direct or shared-infrastructure connection between two entities.
pub fn cmd_explain(a: &str, b: &str, storage: &Arc<dyn StorageEngine>) -> Result<()> {
    let records = storage.read_all_records()?;
    let report = explain_connection(&records, a, b);

    println!("Connection: {} <-> {}", report.entity_a, report.entity_b);
    println!("{}", report.summary);

    for rel in &report.direct_edges {
        println!(
            "  direct: {} --[{}]--> {} ({}, obs: {})",
            rel.source,
            rel.kind,
            rel.target,
            rel.signal,
            rel.evidence.observation_ids.join(", ")
        );
    }

    for path in &report.shared_paths {
        let timing = if path.concurrent {
            "concurrent"
        } else {
            "historical-only"
        };
        println!(
            "  shared path [{}]: {} --[{}]--> {} <--[{}]-- {} (obs: {} / {})",
            timing,
            path.left.source,
            path.left.kind,
            path.left.target,
            path.right.kind,
            path.right.source,
            path.left.evidence.observation_ids.join(", "),
            path.right.evidence.observation_ids.join(", ")
        );
    }

    Ok(())
}

/// Display chronological relationship history for `target`.
pub fn cmd_history(target: &str, storage: &Arc<dyn StorageEngine>) -> Result<()> {
    let records = storage.read_all_records()?;
    let history = entity_history(&records, target);

    println!("Relationship history for: {target}");
    if history.is_empty() {
        println!("  No relationships recorded.");
        return Ok(());
    }

    for rel in &history {
        let state = if rel.is_current { "current" } else { "past" };
        println!(
            "  [{}..{}] ({}) {} --[{}]--> {} ({} obs)",
            rel.evidence.first_seen.format("%Y-%m-%d %H:%M"),
            rel.evidence.last_seen.format("%Y-%m-%d %H:%M"),
            state,
            rel.source,
            rel.kind,
            rel.target,
            rel.evidence.observation_ids.len()
        );
    }

    let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
    let baseline = project_relationships_before(&records, Some(cutoff));
    let current = project_relationships_before(&records, None);
    let complete = latest_scan_is_complete(&records);
    let norm = normalize_fqdn(target);
    let changes: Vec<_> = compare_relationships(&baseline, &current, complete)
        .into_iter()
        .filter(|c| c.relationship.source.id == norm || c.relationship.target.id == norm)
        .collect();
    if !changes.is_empty() {
        println!("\nRecent relationship changes (last 24h):");
        for ch in &changes {
            let label = match ch.change_kind {
                RelationshipChangeKind::Added => "added",
                RelationshipChangeKind::VerifiedRemoval => "verified-removal",
                RelationshipChangeKind::NotObserved => "not-observed",
            };
            println!("  [{}] {}", label, ch.detail);
        }
    }

    Ok(())
}

/// Run bounded local pivot discovery from `target` without active probing.
pub fn cmd_pivot(
    target: &str,
    max_depth: usize,
    max_candidates: usize,
    scope_domains: &[String],
    storage: &Arc<dyn StorageEngine>,
) -> Result<()> {
    let records = storage.read_all_records()?;
    let approved_domains = scope_domains
        .iter()
        .map(|s| normalize_fqdn(s))
        .filter(|s| !s.is_empty())
        .collect();
    let config = PivotConfig {
        approved_domains,
        max_depth,
        max_candidates,
        ..Default::default()
    };
    let report = discover_pivots(&records, target, &config);

    println!(
        "Pivot candidates for: {} (depth <= {}, limit {}, suppressed {} shared-provider clue(s))",
        report.seed, max_depth, max_candidates, report.suppressed_count
    );
    if report.candidates.is_empty() {
        println!("  No candidate entities discovered.");
        return Ok(());
    }

    for c in &report.candidates {
        let scope = if c.in_scope {
            "in-scope"
        } else {
            "out-of-scope (passive review)"
        };
        println!(
            "  {}  [depth {}, {}, {}] — {} (obs: {})",
            c.candidate,
            c.depth,
            scope,
            c.signal,
            c.reason,
            c.provenance_observation_ids.join(", ")
        );
    }
    if report.truncated_by_limit {
        println!("  (Truncated at limit {max_candidates}; pass --limit to inspect more.)");
    }
    Ok(())
}

/// Print registered discovery and enrichment sources and their constraints.
pub fn cmd_sources_status() {
    println!("Discovery & enrichment sources:");
    for src in registered_sources() {
        println!(
            "  {:<12} [{} | api {} | creds: {} | scope_req: {}] — {}",
            src.id,
            src.mode,
            src.api_version,
            if src.requires_credentials {
                "optional/required"
            } else {
                "none"
            },
            src.scope_required,
            src.limitations
        );
    }
}

/// Run offline/cached CT or Passive DNS enrichment from a JSON fixture/file
/// (or print guidance for live fetch).
pub fn cmd_discover(
    source: &str,
    domain: &str,
    input_file: Option<&str>,
    scope_domains: &[String],
    storage: &Arc<dyn StorageEngine>,
) -> Result<()> {
    let path = input_file.ok_or_else(|| {
        BalebosError::Validation(format!(
            "discover --source {source} requires --input <json-file> for offline/cached ingestion"
        ))
    })?;
    let bytes = std::fs::read(path)?;
    let scope = scope_domains
        .iter()
        .map(|s| normalize_fqdn(s))
        .filter(|s| !s.is_empty())
        .collect();

    let candidates = match source {
        "ct" => parse_ct_json(&bytes, &scope, 200)?,
        "passive-dns" => parse_passive_dns_json(&bytes, &scope, 200)?,
        other => {
            return Err(BalebosError::Validation(format!(
                "unknown discovery source '{other}' (expected 'ct' or 'passive-dns')"
            )));
        }
    };

    let records = storage.read_all_records()?;
    let novel = novel_candidates(&records, &candidates);
    let to_persist: Vec<EnrichmentCandidateRecord> = novel
        .iter()
        .map(|c| EnrichmentCandidateRecord {
            item_type: ItemType::EnrichmentCandidate,
            id: uuid::Uuid::new_v4().to_string(),
            candidate: (*c).clone(),
        })
        .collect();
    if !to_persist.is_empty() {
        storage.save_batch(&to_persist)?;
    }

    println!(
        "Enrichment from {source} for {domain}: {} total candidate(s), {} novel persisted",
        candidates.len(),
        to_persist.len()
    );
    for c in novel {
        let scope_label = if c.in_scope {
            "in-scope"
        } else {
            "out-of-scope"
        };
        println!(
            "  {} [{} | {} | {}] — {}",
            c.entity, c.source_id, scope_label, c.signal, c.note
        );
    }
    Ok(())
}

/// List persisted enrichment candidates and their review status.
pub fn cmd_candidates_list(storage: &Arc<dyn StorageEngine>) -> Result<()> {
    let records = storage.read_all_records()?;
    let mut latest_review: std::collections::BTreeMap<String, CandidateReviewDecision> =
        std::collections::BTreeMap::new();
    for r in &records.candidate_reviews {
        latest_review.insert(r.entity_id.clone(), r.decision);
    }

    if records.enrichment_candidates.is_empty() {
        println!("No enrichment candidates recorded.");
        return Ok(());
    }

    println!(
        "Enrichment candidates ({}):",
        records.enrichment_candidates.len()
    );
    for ec in &records.enrichment_candidates {
        let status = match latest_review.get(&ec.candidate.entity.id) {
            Some(CandidateReviewDecision::Confirmed) => "confirmed",
            Some(CandidateReviewDecision::Rejected) => "rejected",
            Some(CandidateReviewDecision::Deferred) => "deferred",
            None => "unreviewed",
        };
        let scope = if ec.candidate.in_scope {
            "in-scope"
        } else {
            "out-of-scope"
        };
        println!(
            "  {}  [{} | {} | {} | {}]",
            ec.candidate.entity, ec.candidate.source_id, scope, status, ec.candidate.signal
        );
    }
    Ok(())
}

/// Persist an operator review decision for a candidate entity.
pub fn cmd_candidates_review(
    entity: &str,
    decision_str: &str,
    storage: &Arc<dyn StorageEngine>,
) -> Result<()> {
    let decision = match decision_str.to_lowercase().as_str() {
        "confirmed" | "confirm" => CandidateReviewDecision::Confirmed,
        "rejected" | "reject" => CandidateReviewDecision::Rejected,
        "deferred" | "defer" => CandidateReviewDecision::Deferred,
        other => {
            return Err(BalebosError::Validation(format!(
                "unknown review decision '{other}' (expected confirmed|rejected|deferred)"
            )));
        }
    };
    let entity_id = normalize_fqdn(entity);
    let rec = CandidateReviewRecord {
        item_type: ItemType::CandidateReview,
        id: uuid::Uuid::new_v4().to_string(),
        entity_id: entity_id.clone(),
        decision,
        reviewed_at: chrono::Utc::now(),
        note: None,
    };
    storage.save(&rec)?;
    println!("Recorded candidate review for {entity_id}: {decision:?}");
    Ok(())
}

/// Print discovery effectiveness metrics for the current store.
pub fn cmd_discovery_report(json_output: bool, storage: &Arc<dyn StorageEngine>) -> Result<()> {
    let records = storage.read_all_records()?;
    let enrichment: Vec<_> = records
        .enrichment_candidates
        .iter()
        .map(|r| r.candidate.clone())
        .collect();
    let mut reviews = std::collections::BTreeMap::new();
    for r in &records.candidate_reviews {
        reviews.insert(r.entity_id.clone(), r.decision);
    }
    let report = compute_discovery_metrics(&records, &[], &enrichment, &reviews);
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
        return Ok(());
    }
    println!(
        "Discovery report: {} known asset(s), {} unique candidate(s), {} reviewed, {} relationship(s) ({:.1}% traceable to observations, {} current, {} historical), projection {} ms",
        report.known_assets,
        report.total_unique_candidates,
        report.candidate_review.reviewed_count,
        report.evidence_quality.total_relationships,
        report.evidence_quality.traceable_percentage,
        report.evidence_quality.current_relationships,
        report.evidence_quality.historical_relationships,
        report.projection_latency_ms
    );
    for y in &report.per_source_yield {
        println!(
            "  source {:<14}: {} raw, {} unique new",
            y.source, y.raw_candidates, y.unique_new_candidates
        );
    }
    Ok(())
}
