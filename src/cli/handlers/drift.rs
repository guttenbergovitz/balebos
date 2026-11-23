//! Drift command handler: compare a past evidence window with the current
//! state and print classified drift (ADR 0009).

use std::sync::Arc;

use crate::app::snapshot;
use crate::drift::compare_snapshots;
use crate::error::Result;
use crate::storage::StorageEngine;

/// Report drift over the last `within_hours`: baseline is the latest
/// evidence at the window start, current is everything recorded, with
/// completeness taken from the most recent scan report.
pub fn cmd_drift(within_hours: u64, storage: &Arc<dyn StorageEngine>) -> Result<()> {
    let records = storage.read_all_records()?;
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(within_hours as i64);

    let baseline = snapshot::assemble_view(&records, Some(cutoff));
    let mut current = snapshot::assemble_view(&records, None);
    current.complete = snapshot::latest_scan_is_complete(&records);

    let findings = compare_snapshots(&baseline, &current);

    println!("Drift report (window: last {within_hours} h)");
    println!(
        "Coverage: {}",
        if current.complete {
            "complete (latest scan fully observed)"
        } else {
            "incomplete — absences are 'not observed', not verified change"
        }
    );
    println!();

    if findings.is_empty() {
        println!("No drift detected.");
        return Ok(());
    }

    println!("{:<28} {:<24} EVIDENCE", "ASSET", "CLASS");
    for finding in &findings {
        println!(
            "{:<28} {:<24} {}",
            finding.asset,
            format!("{:?}", finding.class),
            finding.detail
        );
    }
    println!();
    println!("{} finding(s).", findings.len());
    Ok(())
}
