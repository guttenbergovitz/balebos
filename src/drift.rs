//! Drift detection: deterministic classification of differences between
//! two observation views (ADR 0009, slice 4 of the phase plan).
//!
//! The classifier is pure: it operates on explicit per-asset fact sets and
//! coverage flags, never on storage internals. Storage-backed snapshot
//! assembly lands in a later slice; this module is the tested decision
//! engine it will feed.
//!
//! Invariants (ADR 0005/0009):
//! - Absence is interpreted strictly through coverage: an asset missing
//!   from the current view is a verified disappearance only when that view
//!   is complete; otherwise it is "not observed".
//! - A fact unobserved on either side makes that aspect insufficient
//!   evidence — never silently "unchanged", and never blocking other,
//!   comparable aspects of the same asset.
//! - Current-side inspection failure is distinct from disappearance.
//! - Output is deterministic: findings ordered by asset, then by a fixed
//!   class order; identical inputs produce identical output.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// The fixed drift taxonomy (ADR 0009). Serialized forms are stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftClass {
    /// Present in the current view, absent from the baseline.
    NewlyDiscovered,
    /// In the baseline but not observed in an incomplete current view.
    NotObserved,
    /// Missing from a complete current view (method-limited conclusion).
    VerifiedDisappearance,
    /// DNS record set changed.
    DnsChange,
    /// HTTP status or server behaviour changed.
    HttpBehaviourChange,
    /// TLS certificate identity changed.
    TlsChange,
    /// Redirect chain changed.
    RedirectChainChange,
    /// ASN/hosting attribution changed.
    AttributionChange,
    /// Technology fingerprint set changed.
    FingerprintChange,
    /// The current inspection of this asset failed; nothing is concluded.
    InspectionFailure,
    /// A comparable aspect lacks evidence on one side.
    InsufficientEvidence,
}

/// Per-asset observed facts. `None` means "not observed" — distinct from
/// any observed value, including observed errors elsewhere.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetFacts {
    /// Normalised DNS record set (e.g. "A:1.2.3.4").
    pub dns: Option<BTreeSet<String>>,
    /// HTTP status code of the latest probe.
    pub http_status: Option<u16>,
    /// Identified server header/product.
    pub http_server: Option<String>,
    /// TLS certificate identity (serial or fingerprint).
    pub tls_identity: Option<String>,
    /// Normalised redirect chain (hops in order).
    pub redirects: Option<Vec<String>>,
    /// ASN or hosting provider attribution.
    pub attribution: Option<String>,
    /// Technology fingerprint set.
    pub tech: Option<BTreeSet<String>>,
    /// True when the current inspection attempt failed (network/TLS/parse
    /// errors). Absence of facts due to failure is not absence of asset.
    pub inspection_failed: bool,
}

/// One side of a comparison: coverage flag plus per-asset facts.
#[derive(Debug, Clone, Default)]
pub struct SnapshotView {
    /// True when every planned target was inspected (scan complete).
    pub complete: bool,
    pub facts: BTreeMap<String, AssetFacts>,
}

/// A classified difference with human-readable evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftFinding {
    pub asset: String,
    pub class: DriftClass,
    /// Evidence narrative, e.g. "A:1.2.3.4 -> A:5.6.7.8".
    pub detail: String,
}

fn changed<V: PartialEq + std::fmt::Display>(
    findings: &mut Vec<DriftFinding>,
    asset: &str,
    class: DriftClass,
    aspect: &str,
    baseline: Option<&V>,
    current: Option<&V>,
) {
    match (baseline, current) {
        (Some(b), Some(c)) if b != c => findings.push(DriftFinding {
            asset: asset.to_string(),
            class,
            detail: format!("{aspect}: {b} -> {c}"),
        }),
        (Some(_), None) | (None, Some(_)) => findings.push(DriftFinding {
            asset: asset.to_string(),
            class: DriftClass::InsufficientEvidence,
            detail: format!("{aspect}: not observed on one side; difference cannot be established"),
        }),
        _ => {}
    }
}

fn changed_debug<V: PartialEq + std::fmt::Debug>(
    findings: &mut Vec<DriftFinding>,
    asset: &str,
    class: DriftClass,
    aspect: &str,
    baseline: Option<&V>,
    current: Option<&V>,
) {
    match (baseline, current) {
        (Some(b), Some(c)) if b != c => findings.push(DriftFinding {
            asset: asset.to_string(),
            class,
            detail: format!("{aspect}: {b:?} -> {c:?}"),
        }),
        (Some(_), None) | (None, Some(_)) => findings.push(DriftFinding {
            asset: asset.to_string(),
            class: DriftClass::InsufficientEvidence,
            detail: format!("{aspect}: not observed on one side; difference cannot be established"),
        }),
        _ => {}
    }
}

/// Compare two views and classify every difference. Deterministic output:
/// assets in sorted order, aspects in a fixed order per asset.
pub fn compare_snapshots(baseline: &SnapshotView, current: &SnapshotView) -> Vec<DriftFinding> {
    let mut findings = Vec::new();
    let assets: BTreeSet<&str> = baseline
        .facts
        .keys()
        .map(String::as_str)
        .chain(current.facts.keys().map(String::as_str))
        .collect();

    for asset in assets {
        let base = baseline.facts.get(asset);
        let curr = current.facts.get(asset);

        match (base, curr) {
            (None, Some(_)) => findings.push(DriftFinding {
                asset: asset.to_string(),
                class: DriftClass::NewlyDiscovered,
                detail: "asset present in current view, absent from baseline".to_string(),
            }),
            (Some(_), None) => {
                let class = if current.complete {
                    DriftClass::VerifiedDisappearance
                } else {
                    DriftClass::NotObserved
                };
                let detail = if current.complete {
                    "asset absent from a complete current view"
                } else {
                    "asset not observed during an incomplete current scan"
                };
                findings.push(DriftFinding {
                    asset: asset.to_string(),
                    class,
                    detail: detail.to_string(),
                });
            }
            (Some(b), Some(c)) => {
                if c.inspection_failed {
                    findings.push(DriftFinding {
                        asset: asset.to_string(),
                        class: DriftClass::InspectionFailure,
                        detail: "current inspection failed; no conclusion about change".to_string(),
                    });
                    continue;
                }

                changed_debug(
                    &mut findings,
                    asset,
                    DriftClass::DnsChange,
                    "dns",
                    b.dns.as_ref(),
                    c.dns.as_ref(),
                );
                match (b.http_status, c.http_status) {
                    (Some(x), Some(y)) if x != y => findings.push(DriftFinding {
                        asset: asset.to_string(),
                        class: DriftClass::HttpBehaviourChange,
                        detail: format!("http status: {x} -> {y}"),
                    }),
                    (Some(_), None) | (None, Some(_)) => findings.push(DriftFinding {
                        asset: asset.to_string(),
                        class: DriftClass::InsufficientEvidence,
                        detail: "http status: not observed on one side".to_string(),
                    }),
                    _ => {}
                }
                changed(
                    &mut findings,
                    asset,
                    DriftClass::HttpBehaviourChange,
                    "http server",
                    b.http_server.as_ref(),
                    c.http_server.as_ref(),
                );
                changed(
                    &mut findings,
                    asset,
                    DriftClass::TlsChange,
                    "tls",
                    b.tls_identity.as_ref(),
                    c.tls_identity.as_ref(),
                );
                changed_debug(
                    &mut findings,
                    asset,
                    DriftClass::RedirectChainChange,
                    "redirects",
                    b.redirects.as_ref(),
                    c.redirects.as_ref(),
                );
                changed(
                    &mut findings,
                    asset,
                    DriftClass::AttributionChange,
                    "attribution",
                    b.attribution.as_ref(),
                    c.attribution.as_ref(),
                );
                changed_debug(
                    &mut findings,
                    asset,
                    DriftClass::FingerprintChange,
                    "tech",
                    b.tech.as_ref(),
                    c.tech.as_ref(),
                );
            }
            (None, None) => unreachable!("union iteration"),
        }
    }

    findings.sort_by(|a, b| a.asset.cmp(&b.asset).then(a.class.cmp(&b.class)));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> AssetFacts {
        AssetFacts::default()
    }

    fn dns(entries: &[&str]) -> Option<BTreeSet<String>> {
        Some(entries.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn absence_follows_coverage() {
        let mut baseline = SnapshotView::default();
        baseline.facts.insert("a.example.com".to_string(), facts());

        let incomplete = SnapshotView::default();
        let findings = compare_snapshots(&baseline, &incomplete);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].class, DriftClass::NotObserved);

        let complete = SnapshotView {
            complete: true,
            ..Default::default()
        };
        let findings = compare_snapshots(&baseline, &complete);
        assert_eq!(findings[0].class, DriftClass::VerifiedDisappearance);
    }

    #[test]
    fn newly_discovered_asset() {
        let mut current = SnapshotView::default();
        current.facts.insert("new.example.com".to_string(), facts());
        let findings = compare_snapshots(&SnapshotView::default(), &current);
        assert_eq!(findings[0].class, DriftClass::NewlyDiscovered);
    }

    #[test]
    fn inspection_failure_is_not_disappearance() {
        let mut baseline = SnapshotView::default();
        baseline.facts.insert("a.example.com".to_string(), facts());
        let mut current = SnapshotView {
            complete: true,
            ..Default::default()
        };
        current.facts.insert(
            "a.example.com".to_string(),
            AssetFacts {
                inspection_failed: true,
                ..Default::default()
            },
        );
        let findings = compare_snapshots(&baseline, &current);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].class, DriftClass::InspectionFailure);
    }

    #[test]
    fn each_change_class_is_produced() {
        let base = AssetFacts {
            dns: dns(&["A:1.1.1.1"]),
            http_status: Some(200),
            http_server: Some("nginx".to_string()),
            tls_identity: Some("cert-a".to_string()),
            redirects: Some(vec!["/a".to_string()]),
            attribution: Some("AS1 Example".to_string()),
            tech: dns(&["WordPress"]),
            ..Default::default()
        };
        let curr = AssetFacts {
            dns: dns(&["A:2.2.2.2"]),
            http_status: Some(500),
            http_server: Some("apache".to_string()),
            tls_identity: Some("cert-b".to_string()),
            redirects: Some(vec!["/b".to_string()]),
            attribution: Some("AS2 Other".to_string()),
            tech: dns(&["Drupal"]),
            ..Default::default()
        };
        let mut baseline = SnapshotView::default();
        baseline.facts.insert("a.example.com".to_string(), base);
        let mut current = SnapshotView::default();
        current.facts.insert("a.example.com".to_string(), curr);

        let findings = compare_snapshots(&baseline, &current);
        let classes: BTreeSet<DriftClass> = findings.iter().map(|f| f.class).collect();
        assert!(classes.contains(&DriftClass::DnsChange));
        assert!(classes.contains(&DriftClass::HttpBehaviourChange));
        assert!(classes.contains(&DriftClass::TlsChange));
        assert!(classes.contains(&DriftClass::RedirectChainChange));
        assert!(classes.contains(&DriftClass::AttributionChange));
        assert!(classes.contains(&DriftClass::FingerprintChange));
        assert!(!classes.contains(&DriftClass::InsufficientEvidence));
    }

    #[test]
    fn unobserved_aspect_reports_insufficient_evidence_without_blocking_others() {
        let base = AssetFacts {
            dns: dns(&["A:1.1.1.1"]),
            tls_identity: Some("cert-a".to_string()),
            ..Default::default()
        };
        // Current side observed DNS but not TLS; DNS changed.
        let curr = AssetFacts {
            dns: dns(&["A:9.9.9.9"]),
            ..Default::default()
        };
        let mut baseline = SnapshotView::default();
        baseline.facts.insert("a.example.com".to_string(), base);
        let mut current = SnapshotView::default();
        current.facts.insert("a.example.com".to_string(), curr);

        let findings = compare_snapshots(&baseline, &current);
        let classes: BTreeSet<DriftClass> = findings.iter().map(|f| f.class).collect();
        assert!(classes.contains(&DriftClass::DnsChange));
        assert!(classes.contains(&DriftClass::InsufficientEvidence));
        assert_eq!(
            findings.len(),
            2,
            "one comparable change, one insufficient aspect"
        );
    }

    #[test]
    fn identical_views_produce_no_findings() {
        let f = AssetFacts {
            dns: dns(&["A:1.1.1.1"]),
            http_status: Some(200),
            ..Default::default()
        };
        let mut baseline = SnapshotView::default();
        baseline
            .facts
            .insert("a.example.com".to_string(), f.clone());
        let mut current = SnapshotView::default();
        current.facts.insert("a.example.com".to_string(), f);
        assert!(compare_snapshots(&baseline, &current).is_empty());
    }

    #[test]
    fn output_is_deterministic_and_sorted() {
        let mut baseline = SnapshotView::default();
        baseline.facts.insert("b.example.com".to_string(), facts());
        baseline.facts.insert("a.example.com".to_string(), facts());
        let current = SnapshotView::default();

        let first = compare_snapshots(&baseline, &current);
        let second = compare_snapshots(&baseline, &current);
        assert_eq!(first, second);
        let names: Vec<&str> = first.iter().map(|f| f.asset.as_str()).collect();
        assert_eq!(names, vec!["a.example.com", "b.example.com"]);
    }

    #[test]
    fn drift_class_serde_is_stable() {
        let json = serde_json::to_string(&DriftClass::VerifiedDisappearance).unwrap();
        assert_eq!(json, "\"verified_disappearance\"");
        let back: DriftClass = serde_json::from_str(&json).unwrap();
        assert_eq!(back, DriftClass::VerifiedDisappearance);
    }
}
