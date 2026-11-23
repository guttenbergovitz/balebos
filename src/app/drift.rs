//! Drift report service shared by the CLI and the TUI (ADR 0006).
//!
//! Both surfaces call `drift_findings` with the same records and window,
//! so their results agree by construction.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};

use crate::app::snapshot;
use crate::drift::{compare_snapshots, AssetFacts, DriftFinding};
use crate::storage::reader::Records;

/// Investigation context for a single drift finding: before/after snapshot
/// facts, observation timestamps, contributing sources and latest scan ID.
#[derive(Debug, Clone, Default)]
pub struct DriftEvidence {
    pub previous_summary: String,
    pub current_summary: String,
    pub last_observed: Option<DateTime<Utc>>,
    pub sources: Vec<String>,
    pub latest_scan_id: Option<String>,
}

fn format_facts(facts: Option<&AssetFacts>, empty_label: &str) -> String {
    let Some(f) = facts else {
        return empty_label.to_string();
    };
    let mut parts = Vec::new();
    if let Some(dns) = &f.dns {
        if dns.is_empty() {
            parts.push("DNS: none".to_string());
        } else {
            let joined: Vec<&str> = dns.iter().take(4).map(String::as_str).collect();
            let suffix = if dns.len() > 4 { ", …" } else { "" };
            parts.push(format!("DNS: {}{}", joined.join(", "), suffix));
        }
    }
    if let Some(status) = f.http_status {
        let server = f
            .http_server
            .as_deref()
            .map(|s| format!(" ({s})"))
            .unwrap_or_default();
        parts.push(format!("HTTP {status}{server}"));
    } else if f.inspection_failed {
        parts.push("HTTP: failed".to_string());
    }
    if let Some(attr) = &f.attribution {
        parts.push(format!("Host: {attr}"));
    }
    if parts.is_empty() {
        empty_label.to_string()
    } else {
        parts.join(" · ")
    }
}

/// Compute drift findings for a time window plus the coverage flag of
/// the current view (from the latest scan report).
pub fn drift_findings(records: &Records, within_hours: u64) -> (Vec<DriftFinding>, bool) {
    let (findings, _, complete) = drift_report(records, within_hours);
    (findings, complete)
}

/// Compute drift findings together with per-finding investigation context
/// (before/after facts, timestamps, sources, latest scan ID) and coverage.
pub fn drift_report(
    records: &Records,
    within_hours: u64,
) -> (Vec<DriftFinding>, Vec<DriftEvidence>, bool) {
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(within_hours as i64);
    let baseline = snapshot::assemble_view(records, Some(cutoff));
    let mut current = snapshot::assemble_view(records, None);
    let complete = snapshot::latest_scan_is_complete(records);
    current.complete = complete;

    let findings = compare_snapshots(&baseline, &current);
    let latest_scan_id = records.scan_reports.last().map(|r| r.scan_id.to_string());

    let evidence = findings
        .iter()
        .map(|finding| {
            let mut last_observed: Option<DateTime<Utc>> = None;
            let mut sources = BTreeSet::new();

            for obs in &records.dns_observations {
                if obs.target == finding.asset {
                    last_observed =
                        Some(last_observed.map_or(obs.checked_at, |t| t.max(obs.checked_at)));
                    sources.insert("dnsx".to_string());
                }
            }
            for obs in &records.http_observations {
                if obs.target == finding.asset {
                    last_observed =
                        Some(last_observed.map_or(obs.checked_at, |t| t.max(obs.checked_at)));
                    sources.insert("httpx".to_string());
                }
            }
            for obs in &records.hosting_observations {
                if obs.target == finding.asset {
                    last_observed =
                        Some(last_observed.map_or(obs.checked_at, |t| t.max(obs.checked_at)));
                    sources.insert(obs.detection_method.to_string());
                }
            }
            for item in &records.items {
                if item.name == finding.asset {
                    last_observed = Some(
                        last_observed.map_or(item.discovered_at, |t| t.max(item.discovered_at)),
                    );
                    sources.insert(item.source.clone());
                }
            }

            DriftEvidence {
                previous_summary: format_facts(
                    baseline.facts.get(&finding.asset),
                    "Not observed before cutoff",
                ),
                current_summary: format_facts(
                    current.facts.get(&finding.asset),
                    "Not observed in current window",
                ),
                last_observed,
                sources: sources.into_iter().collect(),
                latest_scan_id: latest_scan_id.clone(),
            }
        })
        .collect();

    (findings, evidence, complete)
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

    #[test]
    fn drift_report_includes_before_after_and_sources() {
        let mut records = Records::default();
        let old = chrono::Utc::now() - chrono::Duration::hours(48);
        let recent = chrono::Utc::now() - chrono::Duration::minutes(5);
        records.items.push(crate::inventory::InventoryItem {
            item_type: crate::inventory::ItemType::Domain,
            id: "1".to_string(),
            name: "api.example.com".to_string(),
            discovered_at: old,
            source: "subfinder".to_string(),
            original_input: None,
        });
        records
            .dns_observations
            .push(crate::app::probe::DnsObservation {
                item_type: crate::inventory::ItemType::DnsRecord,
                id: "d1".to_string(),
                asset_type: "domain".to_string(),
                target: "api.example.com".to_string(),
                a: vec!["192.0.2.10".to_string()],
                aaaa: vec![],
                cname: vec![],
                mx: vec![],
                ns: vec![],
                txt: vec![],
                soa: vec![],
                srv: vec![],
                checked_at: old,
                source: "dnsx".to_string(),
            });
        records
            .dns_observations
            .push(crate::app::probe::DnsObservation {
                item_type: crate::inventory::ItemType::DnsRecord,
                id: "d2".to_string(),
                asset_type: "domain".to_string(),
                target: "api.example.com".to_string(),
                a: vec!["192.0.2.25".to_string()],
                aaaa: vec![],
                cname: vec![],
                mx: vec![],
                ns: vec![],
                txt: vec![],
                soa: vec![],
                srv: vec![],
                checked_at: recent,
                source: "dnsx".to_string(),
            });

        let (findings, evidence, _) = drift_report(&records, 24);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].class, DriftClass::DnsChange);
        assert_eq!(evidence.len(), 1);
        assert!(evidence[0].previous_summary.contains("192.0.2.10"));
        assert!(evidence[0].current_summary.contains("192.0.2.25"));
        assert!(evidence[0].sources.contains(&"dnsx".to_string()));
        assert!(evidence[0].last_observed.is_some());
    }
}
