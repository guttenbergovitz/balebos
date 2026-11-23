//! CLI handlers for evidence-backed relationship queries (`related`,
//! `explain`, `history`).

use std::sync::Arc;

use crate::app::relationships::{entity_history, explain_connection, related_entities};
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
            println!(
                "  {}  [{}] via {}",
                clue.other_entity,
                clue.signal,
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

    for (ea, eb) in &report.shared_paths {
        println!(
            "  shared path: {} --[{}]--> {} <--[{}]-- {} (obs: {} / {})",
            ea.source,
            ea.kind,
            ea.target,
            eb.kind,
            eb.source,
            ea.evidence.observation_ids.join(", "),
            eb.evidence.observation_ids.join(", ")
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

    Ok(())
}
