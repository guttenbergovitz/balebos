//! Drift report service shared by the CLI and the TUI (ADR 0006).
//!
//! Both surfaces call `drift_findings` with the same records and window,
//! so their results agree by construction.

use crate::app::snapshot;
use crate::drift::{compare_snapshots, DriftFinding};
use crate::storage::reader::Records;

/// Compute drift findings for a time window plus the coverage flag of
/// the current view (from the latest scan report).
pub fn drift_findings(records: &Records, within_hours: u64) -> (Vec<DriftFinding>, bool) {
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(within_hours as i64);
    let baseline = snapshot::assemble_view(records, Some(cutoff));
    let mut current = snapshot::assemble_view(records, None);
    let complete = snapshot::latest_scan_is_complete(records);
    current.complete = complete;

    (compare_snapshots(&baseline, &current), complete)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::DriftClass;

    #[test]
    fn service_reports_coverage_and_findings() {
        let records = Records::default();
        let (findings, complete) = drift_findings(&records, 24);
        assert!(findings.is_empty());
        assert!(!complete, "no scan report means incomplete coverage");
    }

    #[test]
    fn service_flags_newly_discovered_assets() {
        let mut records = Records::default();
        records.items.push(crate::inventory::InventoryItem {
            item_type: crate::inventory::ItemType::Domain,
            id: "1".to_string(),
            name: "new.example.com".to_string(),
            discovered_at: chrono::Utc::now(),
            source: "test".to_string(),
            original_input: None,
        });

        let (findings, _) = drift_findings(&records, 24);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].class, DriftClass::NewlyDiscovered);
    }
}
