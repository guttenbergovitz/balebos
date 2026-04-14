//! Risk scoring engine
//!
//! Calculates per-asset risk scores based on findings, lifecycle state,
//! ownership, and staleness. Uses Polars for complex aggregations.

use std::collections::HashMap;

use crate::app::finding::{Finding, FindingCategory, FindingEvent, FindingSeverity};
use crate::inventory::{get_current_lifecycle, AssetLifecycle, LifecycleEvent};
use chrono::{DateTime, Utc};

/// Risk score for a single asset
#[derive(Debug, Clone)]
pub struct RiskScore {
    pub asset_name: String,
    pub total_score: u32,
    pub severity_breakdown: HashMap<FindingSeverity, u32>,
    pub category_breakdown: HashMap<FindingCategory, u32>,
    pub factors: Vec<RiskFactor>,
    pub calculated_at: DateTime<Utc>,
}

/// Individual risk factor contributing to score
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RiskFactor {
    pub description: String,
    pub contribution: i32,
    pub source: RiskFactorSource,
}

/// Source of a risk factor
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum RiskFactorSource {
    Finding { finding_id: String },
    Lifecycle { state: AssetLifecycle },
    Ownership { has_owner: bool },
    Staleness { days: u64 },
}

/// Configuration for risk scoring weights
#[derive(Debug, Clone)]
pub struct RiskWeights {
    pub severity: HashMap<FindingSeverity, u32>,
    pub lifecycle: HashMap<AssetLifecycle, f32>,
    pub no_owner_multiplier: f32,
    pub staleness_multiplier: f32,
    pub staleness_threshold_days: u64,
}

impl Default for RiskWeights {
    fn default() -> Self {
        let mut severity = HashMap::new();
        severity.insert(FindingSeverity::Critical, 100);
        severity.insert(FindingSeverity::High, 50);
        severity.insert(FindingSeverity::Medium, 20);
        severity.insert(FindingSeverity::Low, 5);
        severity.insert(FindingSeverity::Info, 0);

        let mut lifecycle = HashMap::new();
        lifecycle.insert(AssetLifecycle::Active, 1.0);
        lifecycle.insert(AssetLifecycle::UnderReview, 1.2);
        lifecycle.insert(AssetLifecycle::Deprecated, 1.5);
        lifecycle.insert(AssetLifecycle::Decommissioned, 0.5);
        lifecycle.insert(AssetLifecycle::Abandoned, 2.0);

        Self {
            severity,
            lifecycle,
            no_owner_multiplier: 2.0,
            staleness_multiplier: 1.5,
            staleness_threshold_days: 30,
        }
    }
}

/// Calculate risk score for a single asset
pub fn calculate_asset_risk(
    asset_name: &str,
    findings: &[Finding],
    finding_events: &[FindingEvent],
    lifecycle_events: &[LifecycleEvent],
    has_owner: bool,
    days_stale: Option<u64>,
    weights: &RiskWeights,
) -> RiskScore {
    let mut total_score = 0u32;
    let mut severity_breakdown = HashMap::new();
    let mut category_breakdown = HashMap::new();
    let mut factors = Vec::new();

    // Get resolved finding IDs
    let resolved_ids: Vec<&str> = finding_events
        .iter()
        .filter(|e| matches!(e.event_type, crate::app::FindingEventType::Resolved))
        .map(|e| e.finding_id.as_str())
        .collect();

    // Base score from open findings
    for finding in findings.iter().filter(|f| f.target == asset_name) {
        // Skip resolved findings
        if resolved_ids.contains(&finding.id.as_str()) {
            continue;
        }

        let weight = *weights.severity.get(&finding.severity).unwrap_or(&0);
        if weight > 0 {
            total_score += weight;
            *severity_breakdown.entry(finding.severity).or_insert(0) += weight;
            *category_breakdown.entry(finding.category).or_insert(0) += weight;
            factors.push(RiskFactor {
                description: format!(
                    "{} finding: {} ({})",
                    finding.severity, finding.check_name, finding.message
                ),
                contribution: weight as i32,
                source: RiskFactorSource::Finding {
                    finding_id: finding.id.clone(),
                },
            });
        }
    }

    // Apply lifecycle multiplier
    let lifecycle = get_current_lifecycle(lifecycle_events, asset_name);
    let lifecycle_multiplier = lifecycle
        .and_then(|s| weights.lifecycle.get(&s).copied())
        .unwrap_or(1.0);

    if lifecycle_multiplier != 1.0 {
        let original = total_score;
        total_score = (total_score as f32 * lifecycle_multiplier) as u32;
        let extra = total_score.saturating_sub(original);
        if let Some(state) = lifecycle {
            factors.push(RiskFactor {
                description: format!("Lifecycle state: {} (x{})", state, lifecycle_multiplier),
                contribution: extra as i32,
                source: RiskFactorSource::Lifecycle { state },
            });
        }
    }

    // No owner multiplier
    if !has_owner {
        let original = total_score;
        total_score = (total_score as f32 * weights.no_owner_multiplier) as u32;
        let extra = total_score.saturating_sub(original);
        factors.push(RiskFactor {
            description: format!("No owner assigned (x{})", weights.no_owner_multiplier),
            contribution: extra as i32,
            source: RiskFactorSource::Ownership { has_owner: false },
        });
    }

    // Staleness multiplier
    if let Some(days) = days_stale {
        if days >= weights.staleness_threshold_days {
            let original = total_score;
            total_score = (total_score as f32 * weights.staleness_multiplier) as u32;
            let extra = total_score.saturating_sub(original);
            factors.push(RiskFactor {
                description: format!(
                    "Stale asset ({} days >= {} threshold, x{})",
                    days, weights.staleness_threshold_days, weights.staleness_multiplier
                ),
                contribution: extra as i32,
                source: RiskFactorSource::Staleness { days },
            });
        }
    }

    RiskScore {
        asset_name: asset_name.to_string(),
        total_score,
        severity_breakdown,
        category_breakdown,
        factors,
        calculated_at: Utc::now(),
    }
}

/// Calculate risk scores for all assets
pub fn calculate_all_risks(
    findings: &[Finding],
    finding_events: &[FindingEvent],
    lifecycle_events: &[LifecycleEvent],
    asset_names: &[String],
    asset_to_owner: &HashMap<String, bool>,
    asset_to_staleness: &HashMap<String, u64>,
    weights: Option<RiskWeights>,
) -> Vec<RiskScore> {
    let weights = weights.unwrap_or_default();

    let mut scores: Vec<RiskScore> = asset_names
        .iter()
        .map(|name| {
            calculate_asset_risk(
                name,
                findings,
                finding_events,
                lifecycle_events,
                *asset_to_owner.get(name).unwrap_or(&false),
                asset_to_staleness.get(name).copied(),
                &weights,
            )
        })
        .collect();

    // Sort by score descending
    scores.sort_by(|a, b| b.total_score.cmp(&a.total_score));
    scores
}

/// Get risk level color for a score
pub fn risk_color(score: u32) -> ratatui::style::Color {
    use ratatui::style::Color;
    match score {
        0..=24 => Color::Green,
        25..=49 => Color::Yellow,
        50..=99 => Color::Rgb(255, 165, 0), // Orange
        100..=199 => Color::Red,
        _ => Color::Rgb(139, 0, 0), // Dark red
    }
}

/// Get risk level label for a score
pub fn risk_label(score: u32) -> &'static str {
    match score {
        0..=24 => "Low",
        25..=49 => "Medium",
        50..=99 => "High",
        100..=199 => "Critical",
        _ => "Severe",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::finding::{Finding, FindingCategory};
    use crate::inventory::LifecycleEvent;

    #[test]
    fn test_calculate_asset_risk_basic() {
        let finding = Finding::new(
            FindingSeverity::High,
            FindingCategory::Security,
            "test_check",
            "example.com",
            "Test finding",
        );

        let score = calculate_asset_risk(
            "example.com",
            &[finding],
            &[],
            &[],
            true,
            None,
            &RiskWeights::default(),
        );

        assert_eq!(score.total_score, 50);
        assert_eq!(score.factors.len(), 1);
    }

    #[test]
    fn test_lifecycle_multiplier() {
        let event = LifecycleEvent::new(
            "example.com".to_string(),
            None,
            AssetLifecycle::Abandoned,
            None,
            "test".to_string(),
        );

        let score = calculate_asset_risk(
            "example.com",
            &[],
            &[],
            &[event],
            true,
            None,
            &RiskWeights::default(),
        );

        assert_eq!(score.total_score, 0); // No findings, so multiplier has no effect
    }

    #[test]
    fn test_no_owner_multiplier() {
        let finding = Finding::new(
            FindingSeverity::Medium,
            FindingCategory::Operational,
            "test_check",
            "example.com",
            "Test finding",
        );

        let score = calculate_asset_risk(
            "example.com",
            &[finding],
            &[],
            &[],
            false,
            None,
            &RiskWeights::default(),
        );

        // Base 20 * no_owner 2.0 = 40
        assert_eq!(score.total_score, 40);
    }

    #[test]
    fn test_staleness_multiplier() {
        let finding = Finding::new(
            FindingSeverity::Low,
            FindingCategory::Lifecycle,
            "test_check",
            "example.com",
            "Test finding",
        );

        let score = calculate_asset_risk(
            "example.com",
            &[finding],
            &[],
            &[],
            true,
            Some(45), // 45 days stale
            &RiskWeights::default(),
        );

        // Base 5 * stale 1.5 = 7 (rounded down)
        assert_eq!(score.total_score, 7);
    }

    #[test]
    fn test_resolved_findings_not_counted() {
        let finding = Finding::new(
            FindingSeverity::Critical,
            FindingCategory::Security,
            "test_check",
            "example.com",
            "Test finding",
        );
        let finding_id = finding.id.clone();

        let resolved_event = crate::app::finding::FindingEvent::resolve(&finding_id);

        let score = calculate_asset_risk(
            "example.com",
            &[finding],
            &[resolved_event],
            &[],
            true,
            None,
            &RiskWeights::default(),
        );

        assert_eq!(score.total_score, 0);
    }
}
