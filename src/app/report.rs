use crate::app::{check_inventory, get_domain_stats, get_inventory_diff, Severity};
use crate::storage::JsonlStorage;
use std::io;

pub struct Report {
    // Stats
    pub total_domains: usize,
    pub unique_domains: usize,

    // Changes
    pub new_domains: usize,
    pub missing_domains: usize,

    // Issues
    pub critical_issues: usize,
    pub warning_issues: usize,

    // Categories of issues
    pub stale_domains: usize,
    pub low_confidence_domains: usize,
    pub unknown_sources: usize,
    pub invalid_data: usize,

    // Attention level
    pub needs_attention: bool,
}

pub fn generate_report(storage: &JsonlStorage) -> Result<Report, io::Error> {
    // Get stats
    let stats = get_domain_stats(storage)?;

    // Get changes
    let diff = get_inventory_diff(storage, None)?;

    // Get issues
    let issues = check_inventory(storage)?;

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

    // Determine if attention is needed
    let needs_attention = critical_issues > 0
        || warning_issues >= 2
        || !diff.new_domains.is_empty()
        || !diff.missing_domains.is_empty();

    Ok(Report {
        total_domains: stats.total_entries,
        unique_domains: stats.unique_domains,

        new_domains: diff.new_domains.len(),
        missing_domains: diff.missing_domains.len(),

        critical_issues,
        warning_issues,

        stale_domains,
        low_confidence_domains,
        unknown_sources,
        invalid_data,

        needs_attention,
    })
}
