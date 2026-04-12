use crate::app::{check_inventory, get_domain_stats, get_inventory_diff, Severity};
use crate::error::Result;
use crate::storage::StorageEngine;
use std::sync::Arc;

pub struct Report {
    // Stats
    pub total_domains: usize,
    pub unique_domains: usize,

    // Domain Changes
    pub new_domains: usize,
    pub missing_domains: usize,

    // Subdomain Changes
    pub new_subdomains: usize,
    pub missing_subdomains: usize,

    // Issues
    pub critical_issues: usize,
    pub warning_issues: usize,

    // Categories of issues
    pub stale_domains: usize,
    pub low_confidence_domains: usize,
    pub unknown_sources: usize,
    pub invalid_data: usize,
    pub suspicious_subdomains: usize,

    // HTTP issues
    pub http_errors: usize,
    pub missing_http_checks: usize,

    // Findings statistics
    pub open_findings: usize,
    pub acknowledged_findings: usize,
    pub resolved_findings: usize,

    // Attention level
    pub needs_attention: bool,
}

pub fn generate_report(
    storage: &Arc<dyn StorageEngine>,
    ignore_patterns: &[String],
) -> Result<Report> {
    // Get stats
    let stats = get_domain_stats(storage)?;

    // Get changes
    let diff = get_inventory_diff(storage, None)?;

    // Get issues
    let issues = check_inventory(storage, ignore_patterns)?;

    // Count issues by severity
    let critical_issues = issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .count();
    let warning_issues = issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .count();

    // Count specific issue types
    let stale_domains = issues
        .iter()
        .filter(|i| i.message.contains("last seen"))
        .count();

    let low_confidence_domains = issues
        .iter()
        .filter(|i| i.message.contains("seen only once"))
        .count();

    let unknown_sources = issues
        .iter()
        .filter(|i| i.message.contains("Unknown source"))
        .count();

    let invalid_data = issues
        .iter()
        .filter(|i| {
            i.message.contains("is not normalized") || i.message.contains("contains uppercase")
        })
        .count();

    let suspicious_subdomains = issues
        .iter()
        .filter(|i| i.message.contains("Suspicious subdomain"))
        .count();

    let http_errors = issues
        .iter()
        .filter(|i| i.message.contains("HTTP status"))
        .count();

    let missing_http_checks = issues
        .iter()
        .filter(|i| {
            i.message.contains("No HTTP checks") || i.message.contains("No recent HTTP check")
        })
        .count();

    // Get findings statistics
    let records = storage.read_all_records()?;
    let open_findings = records
        .findings
        .iter()
        .filter(|f| {
            !records.finding_events.iter().any(|e| {
                e.finding_id == f.id
                    && e.event_type == crate::app::finding::FindingEventType::Resolved
            })
        })
        .count();

    let acknowledged_findings = records
        .finding_events
        .iter()
        .filter(|e| e.event_type == crate::app::finding::FindingEventType::Acknowledged)
        .count();

    let resolved_findings = records
        .finding_events
        .iter()
        .filter(|e| e.event_type == crate::app::finding::FindingEventType::Resolved)
        .count();

    // Determine if attention is needed (include open findings)
    let needs_attention = critical_issues > 0
        || warning_issues >= 2
        || !diff.new_domains.is_empty()
        || !diff.missing_domains.is_empty()
        || !diff.new_subdomains.is_empty()
        || suspicious_subdomains > 0
        || http_errors > 0
        || open_findings > 0;

    Ok(Report {
        total_domains: stats.total_entries,
        unique_domains: stats.unique_domains,

        new_domains: diff.new_domains.len(),
        missing_domains: diff.missing_domains.len(),

        new_subdomains: diff.new_subdomains.len(),
        missing_subdomains: diff.missing_subdomains.len(),

        critical_issues,
        warning_issues,

        stale_domains,
        low_confidence_domains,
        unknown_sources,
        invalid_data,
        suspicious_subdomains,
        http_errors,
        missing_http_checks,

        open_findings,
        acknowledged_findings,
        resolved_findings,

        needs_attention,
    })
}
