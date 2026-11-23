//! CLI handlers for evidence-backed relationship queries (`related`,
//! `explain`, `history`).

use std::sync::Arc;

use crate::app::pivot::{discover_pivots, PivotConfig};
use crate::app::relationships::{
    compare_relationships, entity_history, explain_connection, project_relationships_before,
    related_entities, RelationshipChangeKind,
};
use crate::app::snapshot::latest_scan_is_complete;
use crate::domain::normalize::normalize_domain;
use crate::error::Result;
use crate::storage::StorageEngine;

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
    let norm = normalize_domain(target.trim().trim_end_matches('.'));
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
        .map(|s| normalize_domain(s.trim()))
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
