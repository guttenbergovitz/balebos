//! Snapshot assembly: materialise a drift `SnapshotView` from typed
//! records (ADR 0009). Pure over `Records` — storage-agnostic and fully
//! fixture-testable. Latest observation per target at (or before) an
//! optional cutoff wins; earlier evidence is never modified.
//!
//! Completeness is deliberately not derived here: assembly reports facts,
//! coverage comes from scan reports (see `latest_scan_is_complete`) and is
//! attached by the caller.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};

use crate::domain::coverage::{ScanReport, TargetCoverage};
use crate::drift::SnapshotView;
use crate::storage::reader::Records;

/// Assemble a snapshot view from typed records.
///
/// `before` bounds the evidence window: only observations at or before the
/// cutoff participate. `None` means "everything recorded".
pub fn assemble_view(records: &Records, before: Option<DateTime<Utc>>) -> SnapshotView {
    let mut view = SnapshotView::default();

    // Every known asset appears, even without observations: absence of
    // facts is information the classifier reasons about. Inventory items
    // respect the evidence window via their discovery time.
    for item in &records.items {
        if before.is_some_and(|cutoff| item.discovered_at > cutoff) {
            continue;
        }
        view.facts.entry(item.name.clone()).or_default();
    }

    // DNS: latest observation per target within the window.
    let mut latest_dns: std::collections::HashMap<String, &crate::app::probe::DnsObservation> =
        std::collections::HashMap::new();
    for obs in &records.dns_observations {
        if before.is_some_and(|cutoff| obs.checked_at > cutoff) {
            continue;
        }
        let entry = latest_dns.entry(obs.target.clone()).or_insert(obs);
        if obs.checked_at > entry.checked_at {
            *entry = obs;
        }
    }
    for (target, obs) in latest_dns {
        let facts = view.facts.entry(target).or_default();
        let mut set = BTreeSet::new();
        for ip in &obs.a {
            set.insert(format!("A:{ip}"));
        }
        for ip in &obs.aaaa {
            set.insert(format!("AAAA:{ip}"));
        }
        for name in &obs.cname {
            set.insert(format!("CNAME:{name}"));
        }
        facts.dns = Some(set);
    }

    // HTTP: latest observation per target within the window.
    let mut latest_http: std::collections::HashMap<String, &crate::app::probe::HttpObservation> =
        std::collections::HashMap::new();
    for obs in &records.http_observations {
        if before.is_some_and(|cutoff| obs.checked_at > cutoff) {
            continue;
        }
        let entry = latest_http.entry(obs.target.clone()).or_insert(obs);
        if obs.checked_at > entry.checked_at {
            *entry = obs;
        }
    }
    for (target, obs) in latest_http {
        let facts = view.facts.entry(target).or_default();
        facts.http_status = obs.status;
        facts.http_server = obs.webserver.clone();
        facts.tls_identity = obs
            .tls
            .as_ref()
            .map(|tls| format!("{}|{}|{}", tls.subject_dn, tls.issuer_dn, tls.not_after));
        facts.redirects = Some(
            obs.redirect_chain
                .iter()
                .map(|hop| hop.url.clone())
                .collect(),
        );
        facts.tech = Some(obs.tech.iter().cloned().collect());
        facts.inspection_failed = obs.status.is_none() && obs.error_kind.is_some();
    }

    // Attribution: hosting provider wins, ASN as fallback.
    let mut latest_hosting: std::collections::HashMap<
        String,
        &crate::app::probe::HostingObservation,
    > = std::collections::HashMap::new();
    for obs in &records.hosting_observations {
        if before.is_some_and(|cutoff| obs.checked_at > cutoff) {
            continue;
        }
        let entry = latest_hosting.entry(obs.target.clone()).or_insert(obs);
        if obs.checked_at > entry.checked_at {
            *entry = obs;
        }
    }
    for (target, obs) in &latest_hosting {
        if let Some(name) = &obs.provider_name {
            view.facts.entry(target.clone()).or_default().attribution = Some(name.clone());
        }
    }

    let mut latest_asn: std::collections::HashMap<String, &crate::app::probe::AsnObservation> =
        std::collections::HashMap::new();
    for obs in &records.asn_observations {
        if before.is_some_and(|cutoff| obs.checked_at > cutoff) {
            continue;
        }
        let entry = latest_asn.entry(obs.host.clone()).or_insert(obs);
        if obs.checked_at > entry.checked_at {
            *entry = obs;
        }
    }
    for (host, obs) in latest_asn {
        let facts = view.facts.entry(host).or_default();
        if facts.attribution.is_none() {
            facts.attribution = Some(format!("AS{} {}", obs.asn, obs.org));
        }
    }

    view
}

/// True when the most recent scan report proves full coverage: every
/// planned target has an outcome and none failed or went unattempted.
pub fn latest_scan_is_complete(records: &Records) -> bool {
    let Some(report): Option<&ScanReport> = records
        .scan_reports
        .iter()
        .max_by_key(|report| report.started_at)
    else {
        return false;
    };

    report.outcomes.len() == report.planned_targets.len()
        && !report.outcomes.iter().any(|outcome| {
            matches!(
                outcome.coverage,
                TargetCoverage::Failed { .. } | TargetCoverage::NotAttempted
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::probe::{AsnObservation, DnsObservation, HostingObservation, HttpObservation};
    use crate::domain::coverage::{ScanReport, TargetCoverage, TargetCoverageRecord};
    use crate::inventory::{InventoryItem, ItemType};

    fn item(name: &str) -> InventoryItem {
        InventoryItem {
            item_type: ItemType::Domain,
            id: name.to_string(),
            name: name.to_string(),
            discovered_at: Utc::now(),
            source: "test".to_string(),
            original_input: None,
        }
    }

    fn dns_obs(target: &str, a: &[&str], at: DateTime<Utc>) -> DnsObservation {
        DnsObservation {
            item_type: ItemType::DnsRecord,
            id: uuid::Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target: target.to_string(),
            a: a.iter().map(|s| s.to_string()).collect(),
            aaaa: vec![],
            cname: vec![],
            mx: vec![],
            ns: vec![],
            txt: vec![],
            soa: vec![],
            srv: vec![],
            checked_at: at,
            source: "test".to_string(),
        }
    }

    fn http_obs(target: &str, status: Option<u16>, at: DateTime<Utc>) -> HttpObservation {
        HttpObservation {
            item_type: ItemType::HttpCheck,
            id: uuid::Uuid::new_v4().to_string(),
            asset_type: "domain".to_string(),
            target: target.to_string(),
            status,
            response_time_ms: 10,
            final_url: None,
            redirect_chain: vec![],
            tls: None,
            tech: vec![],
            webserver: None,
            title: None,
            cdn_name: None,
            favicon_hash: None,
            checked_at: at,
            source: "test".to_string(),
            error_kind: None,
            error_message: None,
        }
    }

    #[test]
    fn assembly_latest_observation_wins() {
        let t0 = Utc::now() - chrono::Duration::hours(2);
        let t1 = Utc::now() - chrono::Duration::hours(1);
        let mut records = Records::default();
        records.items.push(item("example.com"));
        records
            .dns_observations
            .push(dns_obs("example.com", &["1.1.1.1"], t0));
        records
            .dns_observations
            .push(dns_obs("example.com", &["2.2.2.2"], t1));

        let view = assemble_view(&records, None);
        let facts = view.facts.get("example.com").unwrap();
        assert_eq!(
            facts.dns,
            Some(["A:2.2.2.2".to_string()].into_iter().collect())
        );
    }

    #[test]
    fn time_boundary_excludes_later_evidence() {
        let old = Utc::now() - chrono::Duration::hours(48);
        let recent = Utc::now() - chrono::Duration::hours(1);
        let mut records = Records::default();
        records.items.push(item("example.com"));
        records
            .dns_observations
            .push(dns_obs("example.com", &["1.1.1.1"], old));
        records
            .http_observations
            .push(http_obs("example.com", Some(200), recent));

        let baseline = assemble_view(&records, Some(Utc::now() - chrono::Duration::hours(24)));
        let facts = baseline.facts.get("example.com").unwrap();
        assert_eq!(
            facts.dns,
            Some(["A:1.1.1.1".to_string()].into_iter().collect())
        );
        assert_eq!(facts.http_status, None, "later evidence excluded");
    }

    #[test]
    fn failed_http_marks_inspection_failed() {
        let mut records = Records::default();
        records.items.push(item("example.com"));
        let mut obs = http_obs("example.com", None, Utc::now());
        obs.error_kind = Some(crate::app::probe::HttpErrorKind::DnsNotFound);
        records.http_observations.push(obs);

        let view = assemble_view(&records, None);
        let facts = view.facts.get("example.com").unwrap();
        assert!(facts.inspection_failed);
    }

    #[test]
    fn completeness_comes_from_latest_scan_report() {
        let mut records = Records::default();
        assert!(!latest_scan_is_complete(&records), "no scan report");

        let full = ScanReport::start(Utc::now()).finish(
            vec!["a.example.com".to_string()],
            vec![TargetCoverageRecord {
                target: "a.example.com".to_string(),
                coverage: TargetCoverage::Observed,
            }],
            Utc::now(),
        );
        records.scan_reports.push(full);
        assert!(latest_scan_is_complete(&records));

        let failed = ScanReport::start(Utc::now()).finish(
            vec!["a.example.com".to_string()],
            vec![TargetCoverageRecord {
                target: "a.example.com".to_string(),
                coverage: TargetCoverage::Failed {
                    error: "boom".to_string(),
                },
            }],
            Utc::now(),
        );
        records.scan_reports.push(failed);
        assert!(
            !latest_scan_is_complete(&records),
            "latest report has a failure"
        );
    }

    #[test]
    fn asn_fills_attribution_when_hosting_absent() {
        let mut records = Records::default();
        records.items.push(item("example.com"));
        records.asn_observations.push(AsnObservation {
            item_type: ItemType::AsnRecord,
            id: uuid::Uuid::new_v4().to_string(),
            target: "1.1.1.1".to_string(),
            host: "example.com".to_string(),
            asn: 16509,
            org: "Amazon".to_string(),
            country: "US".to_string(),
            checked_at: Utc::now(),
            source: "test".to_string(),
        });

        let view = assemble_view(&records, None);
        let facts = view.facts.get("example.com").unwrap();
        assert_eq!(facts.attribution.as_deref(), Some("AS16509 Amazon"));
    }

    #[test]
    fn hosting_wins_over_asn_for_attribution() {
        let mut records = Records::default();
        records.items.push(item("example.com"));
        records.asn_observations.push(AsnObservation {
            item_type: ItemType::AsnRecord,
            id: uuid::Uuid::new_v4().to_string(),
            target: "1.1.1.1".to_string(),
            host: "example.com".to_string(),
            asn: 16509,
            org: "Amazon".to_string(),
            country: "US".to_string(),
            checked_at: Utc::now(),
            source: "test".to_string(),
        });
        records.hosting_observations.push(HostingObservation {
            item_type: ItemType::HostingDetection,
            id: uuid::Uuid::new_v4().to_string(),
            target: "example.com".to_string(),
            provider_id: Some("aws".to_string()),
            provider_name: Some("Amazon Web Services".to_string()),
            provider_category: Some("hyperscaler".to_string()),
            cdn_detected: false,
            cdn_name: None,
            confidence: crate::app::probe::Confidence::High,
            detection_method: crate::app::probe::DetectionMethod::Asn,
            evidence: vec![],
            attribution: None,
            checked_at: Utc::now(),
            source: "test".to_string(),
        });

        let view = assemble_view(&records, None);
        let facts = view.facts.get("example.com").unwrap();
        assert_eq!(facts.attribution.as_deref(), Some("Amazon Web Services"));
    }
}
