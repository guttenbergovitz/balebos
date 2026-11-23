//! Scan coverage: what a scan planned and what happened to each target.
//!
//! Coverage is evidence (ADR 0005), persisted per scan run so later phases
//! can distinguish "not observed this scan" from "verified gone" (ADR 0009).
//! A failed scan is recorded as failed coverage — it never implies that
//! previously known assets disappeared.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::identity::ScanId;

/// What happened to a single planned target during a scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetCoverage {
    /// At least one successful observation was produced for the target.
    Observed,
    /// The target was attempted but every inspection failed; carries a
    /// short error description.
    Failed {
        /// Normalised error from the failing inspection.
        error: String,
    },
    /// The target was deliberately not inspected (ignore pattern or junk).
    Skipped,
    /// The target was valid but never reached (partial scan). Reserved for
    /// scan paths that can abort mid-run.
    NotAttempted,
}

/// Coverage outcome for one planned target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetCoverageRecord {
    pub target: String,
    pub coverage: TargetCoverage,
}

/// An identifiable scan run with explicit scope and per-target outcomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanReport {
    /// Unique identity of this scan run.
    pub scan_id: ScanId,
    /// When the scan started.
    pub started_at: DateTime<Utc>,
    /// When the scan finished.
    pub finished_at: DateTime<Utc>,
    /// The full scope: every target the scan planned to cover, in input
    /// order. Coverage outcomes reference these; the scope is recorded, not
    /// implied.
    pub planned_targets: Vec<String>,
    /// Per-target outcomes. Every planned target appears exactly once.
    pub outcomes: Vec<TargetCoverageRecord>,
}

impl ScanReport {
    /// Begin a report for a scan that just started.
    pub fn start(started_at: DateTime<Utc>) -> ScanStart {
        ScanStart {
            scan_id: ScanId::new(),
            started_at,
        }
    }

    /// Count of planned targets with the given outcome variant.
    pub fn count_coverage(&self, coverage: &TargetCoverage) -> usize {
        self.outcomes
            .iter()
            .filter(|o| std::mem::discriminant(&o.coverage) == std::mem::discriminant(coverage))
            .count()
    }
}

/// Builder state holding the scan identity before targets are known.
pub struct ScanStart {
    scan_id: ScanId,
    started_at: DateTime<Utc>,
}

impl ScanStart {
    /// Finish the report once every planned target has an outcome.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if outcomes do not cover planned targets
    /// exactly (each target exactly once). Production callers validate
    /// scope at the seam; a malformed report is a programming error, not
    /// operator data.
    pub fn finish(
        self,
        planned_targets: Vec<String>,
        outcomes: Vec<TargetCoverageRecord>,
        finished_at: DateTime<Utc>,
    ) -> ScanReport {
        debug_assert_eq!(
            planned_targets.len(),
            outcomes.len(),
            "coverage outcomes must cover every planned target exactly once"
        );
        debug_assert!(
            {
                let mut seen: std::collections::HashSet<&str> =
                    outcomes.iter().map(|o| o.target.as_str()).collect();
                planned_targets.iter().all(|t| seen.remove(t.as_str())) && seen.is_empty()
            },
            "coverage outcomes must match the planned scope exactly"
        );

        ScanReport {
            scan_id: self.scan_id,
            started_at: self.started_at,
            finished_at,
            planned_targets,
            outcomes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(target: &str, coverage: TargetCoverage) -> TargetCoverageRecord {
        TargetCoverageRecord {
            target: target.to_string(),
            coverage,
        }
    }

    #[test]
    fn coverage_variants_serde_round_trip() {
        let cases = [
            (TargetCoverage::Observed, r#""observed""#),
            (
                TargetCoverage::Failed {
                    error: "dns timeout".to_string(),
                },
                r#"{"failed":{"error":"dns timeout"}}"#,
            ),
            (TargetCoverage::Skipped, r#""skipped""#),
            (TargetCoverage::NotAttempted, r#""not_attempted""#),
        ];

        for (coverage, expected) in cases {
            let json = serde_json::to_string(&coverage).unwrap();
            assert_eq!(json, expected, "stable serialised form changed");
            let back: TargetCoverage = serde_json::from_str(&json).unwrap();
            assert_eq!(back, coverage);
        }
    }

    #[test]
    fn report_round_trips_through_json_with_plain_string_scan_id() {
        let report = ScanReport::start(Utc::now()).finish(
            vec!["a.example.com".to_string(), "b.example.com".to_string()],
            vec![
                outcome("a.example.com", TargetCoverage::Observed),
                outcome(
                    "b.example.com",
                    TargetCoverage::Failed {
                        error: "nxdomain".to_string(),
                    },
                ),
            ],
            Utc::now(),
        );

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"scan_id\":\""));

        let back: ScanReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, report);
    }

    #[test]
    fn counts_outcomes_by_variant() {
        let report = ScanReport::start(Utc::now()).finish(
            vec![
                "a.example.com".to_string(),
                "b.example.com".to_string(),
                "c.example.com".to_string(),
            ],
            vec![
                outcome("a.example.com", TargetCoverage::Observed),
                outcome(
                    "b.example.com",
                    TargetCoverage::Failed {
                        error: "x".to_string(),
                    },
                ),
                outcome(
                    "c.example.com",
                    TargetCoverage::Failed {
                        error: "y".to_string(),
                    },
                ),
            ],
            Utc::now(),
        );

        assert_eq!(report.count_coverage(&TargetCoverage::Observed), 1);
        assert_eq!(
            report.count_coverage(&TargetCoverage::Failed {
                error: String::new()
            }),
            2
        );
        assert_eq!(report.count_coverage(&TargetCoverage::Skipped), 0);
    }
}
